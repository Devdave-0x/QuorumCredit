use crate::errors::ContractError;
use crate::helpers::{
    bps_of, config, extend_ttl, get_active_loan_record, get_slash_balance, has_active_loan,
    next_loan_id, require_allowed_token, require_not_paused, require_not_paused_for,
    validate_loan_active,
};
use crate::reputation::ReputationNftExternalClient;
use crate::types::{
    DataKey, LoanCategory, LoanRecord, LoanStatus, PauseFlag, VouchRecord, DEFAULT_REFERRAL_BONUS_BPS,
    MIN_VOUCH_AGE, CANCELLATION_WINDOW_SECONDS,
};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, Vec};

/// ------------------------------
/// Risk Score Computation (#646)
/// ------------------------------
/// Computes a risk score in [0, 10_000] from borrower history.
/// Higher score = higher default risk.
/// Formula: defaults / (loans + 1) * 10_000, capped at 10_000.
pub fn compute_risk_score(env: &Env, borrower: &Address) -> i128 {
    let default_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::DefaultCount(borrower.clone()))
        .unwrap_or(0);
    let loan_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::LoanCount(borrower.clone()))
        .unwrap_or(0);
    let repayment_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::RepaymentCount(borrower.clone()))
        .unwrap_or(0);

    // Penalise defaults, reward repayments
    // risk = (defaults * 10_000) / (loans + repayments + 1), capped at 10_000
    let numerator = (default_count as i128) * 10_000;
    let denominator = (loan_count as i128) + (repayment_count as i128) + 1;
    (numerator / denominator).min(10_000)
}

/// ------------------------------
/// Dynamic Yield Calculation (#646)
/// ------------------------------
/// Adjusts yield_bps upward for high-risk borrowers (up to 2× base)
/// and downward for low-risk borrowers (down to 0.5× base).
fn calculate_dynamic_yield(
    env: &Env,
    borrower: &Address,
    cfg: &crate::types::Config,
) -> i128 {
    let risk_score = compute_risk_score(env, borrower); // 0..10_000
    // risk_score == 0   → multiplier = 5_000 (0.5×)
    // risk_score == 5_000 → multiplier = 10_000 (1.0×)
    // risk_score == 10_000 → multiplier = 20_000 (2.0×)
    let multiplier = 5_000 + risk_score + risk_score / 2; // 5_000..20_000
    let rate = (cfg.yield_bps * multiplier) / 10_000;
    rate.max(1).min(cfg.yield_bps * 2)
}

/// ------------------------------
/// Dynamic Slash Calculation (#646)
/// ------------------------------
/// Adjusts slash_bps upward for high-risk borrowers (up to 2× base).
fn calculate_dynamic_slash(
    env: &Env,
    borrower: &Address,
    cfg: &crate::types::Config,
) -> i128 {
    let risk_score = compute_risk_score(env, borrower); // 0..10_000
    let multiplier = 10_000 + risk_score; // 10_000..20_000
    let rate = (cfg.slash_bps * multiplier) / 10_000;
    rate.min(10_000) // slash_bps capped at 100%
}

/// Register a referrer for a borrower. Must be called before `request_loan`.
pub fn register_referral(
    env: Env,
    borrower: Address,
    referrer: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    assert!(borrower != referrer, "borrower cannot refer themselves");
    assert!(
        !has_active_loan(&env, &borrower),
        "cannot set referral with active loan"
    );

    env.storage()
        .persistent()
        .set(&DataKey::ReferredBy(borrower.clone()), &referrer);

    extend_ttl(&env, &DataKey::ReferredBy(borrower.clone()));

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("set")),
        (borrower, referrer),
    );

    Ok(())
}

pub fn get_referrer(env: Env, borrower: Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::ReferredBy(borrower))
}

/// Backward-compatible request_loan (no co-borrowers, no syndicate).
pub fn request_loan(
    env: Env,
    borrower: Address,
    amount: i128,
    threshold: i128,
    loan_purpose: soroban_sdk::String,
    token_addr: Address,
    syndicate_id: Option<u64>,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;
    require_not_paused_for(&env, PauseFlag::LoanRequest)?;
    let empty: Vec<Address> = Vec::new(&env);
    request_loan_internal(env, borrower, amount, threshold, loan_purpose, token_addr, empty, syndicate_id)
}

/// Task 3: Request a loan with co-borrowers who share repayment responsibility.
pub fn request_loan_with_co_borrowers(
    env: Env,
    borrower: Address,
    amount: i128,
    threshold: i128,
    loan_purpose: soroban_sdk::String,
    token_addr: Address,
    co_borrowers: Vec<Address>,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;
    require_not_paused_for(&env, PauseFlag::LoanRequest)?;

    for i in 0..co_borrowers.len() {
        let cb = co_borrowers.get(i).unwrap();
        cb.require_auth();
        if cb == borrower {
            return Err(ContractError::SelfVouchNotAllowed);
        }
    }

    request_loan_internal(env, borrower, amount, threshold, loan_purpose, token_addr, co_borrowers, None)
}

fn request_loan_internal(
    env: Env,
    borrower: Address,
    amount: i128,
    threshold: i128,
    loan_purpose: soroban_sdk::String,
    token_addr: Address,
    co_borrowers: Vec<Address>,
    syndicate_id: Option<u64>,
) -> Result<(), ContractError> {
    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(borrower.clone()))
        .unwrap_or(false)
    {
        return Err(ContractError::Blacklisted);
    }

    let cfg = config(&env);

    assert!(
        amount >= cfg.min_loan_amount,
        "loan amount must meet minimum threshold"
    );
    assert!(threshold > 0, "threshold must be greater than zero");

    // #643: Validate loan_purpose against allowed_purposes whitelist
    if !cfg.allowed_purposes.is_empty() {
        let purpose_allowed = cfg.allowed_purposes.iter().any(|p| p == loan_purpose);
        if !purpose_allowed {
            return Err(ContractError::LoanPurposeNotAllowed);
        }
    }

    let token_client = require_allowed_token(&env, &token_addr)?;

    let max_loan_amount: i128 = env
        .storage()
        .instance()
        .get(&DataKey::MaxLoanAmount)
        .unwrap_or(0);

    if max_loan_amount > 0 && amount > max_loan_amount {
        return Err(ContractError::LoanExceedsMaxAmount);
    }

    assert!(
        !has_active_loan(&env, &borrower),
        "borrower already has an active loan"
    );

    let now = env.ledger().timestamp();

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    let mut total_stake: i128 = 0;
    for v in vouches.iter() {
        total_stake += v.amount;
    }

    if total_stake < threshold {
        panic_with_error!(&env, ContractError::InsufficientFunds);
    }

    // #642: Enforce sector diversification — no single sector may contribute > 50% of total stake
    if total_stake > 0 {
        // Count stake per sector
        let mut sector_stakes: Vec<(soroban_sdk::String, i128)> = Vec::new(&env);
        for v in vouches.iter() {
            let mut found = false;
            for i in 0..sector_stakes.len() {
                let (s, stake_val) = sector_stakes.get(i).unwrap();
                if s == v.sector {
                    sector_stakes.set(i, (s, stake_val + v.amount));
                    found = true;
                    break;
                }
            }
            if !found {
                sector_stakes.push_back((v.sector.clone(), v.amount));
            }
        }
        for i in 0..sector_stakes.len() {
            let (_, sector_stake) = sector_stakes.get(i).unwrap();
            // Reject if any sector contributes more than 50% of total stake
            if sector_stake * 2 > total_stake {
                return Err(ContractError::SectorConcentrationTooHigh);
            }
        }
    }

    let deadline = now + cfg.loan_duration;

    let loan_id = next_loan_id(&env);

    // ------------------------------
    // DYNAMIC YIELD (#646)
    // ------------------------------
    let yield_bps = calculate_dynamic_yield(&env, &borrower, &cfg);
    let dynamic_slash_bps = calculate_dynamic_slash(&env, &borrower, &cfg);

    let total_yield = bps_of(amount, yield_bps);

    // #644: Collect insurance premium if configured
    if cfg.insurance_premium_bps > 0 {
        let premium = bps_of(amount, cfg.insurance_premium_bps);
        if premium > 0 {
            // Transfer premium from borrower to contract (held in insurance pool)
            token_client.transfer(&borrower, &env.current_contract_address(), &premium);
            // Add to insurance pool balance
            let pool_key = DataKey::InsurancePool;
            let pool_balance: i128 = env.storage().instance().get(&pool_key).unwrap_or(0);
            env.storage().instance().set(&pool_key, &(pool_balance + premium));
        }
    }

    // Default to Personal category if not specified
    let loan_category = LoanCategory::Personal;

    env.storage().persistent().set(
        &DataKey::Loan(loan_id),
        &LoanRecord {
            id: loan_id,
            borrower: borrower.clone(),
            co_borrowers,
            amount,
            amount_repaid: 0,
            total_yield,
            yield_bps,
            slash_bps: dynamic_slash_bps,
            status: LoanStatus::Active,
            created_at: now,
            disbursement_timestamp: now,
            repayment_timestamp: None,
            deadline,
            loan_purpose,
            loan_category: loan_category.clone(),
            token_address: token_addr.clone(),
            syndicate_id,
        },
    );

    extend_ttl(&env, &DataKey::Loan(loan_id));

    // #647: Track loan in syndicate if syndicate_id provided
    if let Some(sid) = syndicate_id {
        let mut syndicate_loans: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::Syndicate(sid))
            .unwrap_or(Vec::new(&env));
        syndicate_loans.push_back(loan_id);
        env.storage()
            .persistent()
            .set(&DataKey::Syndicate(sid), &syndicate_loans);
        extend_ttl(&env, &DataKey::Syndicate(sid));
    }

    // Task 4: Track loan by category
    let mut category_loans: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::LoanCategoryLoans(loan_category.clone()))
        .unwrap_or(Vec::new(&env));
    category_loans.push_back(loan_id);
    env.storage()
        .persistent()
        .set(&DataKey::LoanCategoryLoans(loan_category.clone()), &category_loans);
    extend_ttl(&env, &DataKey::LoanCategoryLoans(loan_category));
    env.storage()
        .persistent()
        .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);

    extend_ttl(&env, &DataKey::ActiveLoan(borrower.clone()));

    env.storage()
        .persistent()
        .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

    extend_ttl(&env, &DataKey::LatestLoan(borrower.clone()));

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::LoanCount(borrower.clone()))
        .unwrap_or(0);

    env.storage()
        .persistent()
        .set(&DataKey::LoanCount(borrower.clone()), &(count + 1));

    extend_ttl(&env, &DataKey::LoanCount(borrower.clone()));

    token_client.transfer(&env.current_contract_address(), &borrower, &amount);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("disbursed")),
        (borrower.clone(), amount, deadline, token_addr),
    );

    Ok(())
}

/* --------------------------------------------------
   Everything below remains unchanged (repay, views)
-------------------------------------------------- */

pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;
    require_not_paused_for(&env, PauseFlag::Repay)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if borrower != loan.borrower {
        return Err(ContractError::UnauthorizedCaller);
    }

    validate_loan_active(&loan)?;

    let total_owed = loan.amount + loan.total_yield;
    let outstanding = total_owed - loan.amount_repaid;

    if payment <= 0 || payment > outstanding {
        return Err(ContractError::InvalidAmount);
    }

    let token = soroban_sdk::token::Client::new(&env, &loan.token_address);
    token.transfer(&borrower, &env.current_contract_address(), &payment);
    loan.amount_repaid += payment;

    if loan.amount_repaid >= total_owed {
        loan.status = LoanStatus::Repaid;
        loan.repayment_timestamp = Some(env.ledger().timestamp());
    }

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    Ok(())
}

pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
    match crate::helpers::get_latest_loan_record(&env, &borrower) {
        None => LoanStatus::None,
        Some(loan) => loan.status,
    }
}

pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
    crate::helpers::get_latest_loan_record(&env, &borrower)
}

pub fn get_loan_by_id(env: Env, loan_id: u64) -> Option<LoanRecord> {
    env.storage().persistent().get(&DataKey::Loan(loan_id))
}

pub fn get_loan_status(env: Env, loan_id: u64) -> LoanStatus {
    env.storage()
        .persistent()
        .get::<DataKey, LoanRecord>(&DataKey::Loan(loan_id))
        .map(|l| l.status)
        .unwrap_or(LoanStatus::None)
}

pub fn is_eligible(env: Env, borrower: Address, threshold: i128) -> bool {
    if threshold <= 0 {
        return false;
    }
    if let Some(loan) = crate::helpers::get_latest_loan_record(&env, &borrower) {
        if loan.status == LoanStatus::Active {
            return false;
        }
    }
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));
    let total_stake: i128 = vouches.iter().map(|v| v.amount).sum();
    total_stake >= threshold
}

pub fn get_loan_purpose(env: Env, loan_id: u64) -> Option<soroban_sdk::String> {
    env.storage()
        .persistent()
        .get::<DataKey, LoanRecord>(&DataKey::Loan(loan_id))
        .map(|l| l.loan_purpose)
}

pub fn repayment_count(env: Env, borrower: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::RepaymentCount(borrower))
        .unwrap_or(0)
}

pub fn loan_count(env: Env, borrower: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::LoanCount(borrower))
        .unwrap_or(0)
}

pub fn default_count(env: Env, borrower: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::DefaultCount(borrower))
        .unwrap_or(0)
}

// Task 1: Loan Cancellation - Allow borrower to cancel loan before disbursement
pub fn cancel_loan(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let loan = get_active_loan_record(&env, &borrower)?;

    // Only allow cancellation if loan is Pending or within cancellation window of Active
    let now = env.ledger().timestamp();
    match loan.status {
        LoanStatus::Pending => {
            // Can always cancel pending loans
        }
        LoanStatus::Active => {
            // Can only cancel within 1 hour of disbursement
            if now > loan.disbursement_timestamp + CANCELLATION_WINDOW_SECONDS {
                return Err(ContractError::CancellationWindowExpired);
            }
        }
        _ => {
            return Err(ContractError::LoanNotCancellable);
        }
    }

    // Return all voucher stakes immediately
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    for v in vouches.iter() {
        let token_client = soroban_sdk::token::Client::new(&env, &v.token);
        token_client.transfer(&env.current_contract_address(), &v.voucher, &v.amount);
    }

    // Remove vouches and loan records
    env.storage()
        .persistent()
        .remove(&DataKey::Vouches(borrower.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::ActiveLoan(borrower.clone()));

    // Mark loan as cancelled
    let mut updated_loan = loan.clone();
    updated_loan.status = LoanStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &updated_loan);
    extend_ttl(&env, &DataKey::Loan(loan.id));

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("cancelled")),
        (borrower, loan.amount),
    );

    Ok(())
}

// Task 4: Loan Category Analytics - Get all loan IDs by category
pub fn get_loans_by_category(env: Env, category: LoanCategory) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::LoanCategoryLoans(category))
        .unwrap_or(Vec::new(&env))
}

/// #647: Get all loan IDs belonging to a syndicate.
pub fn get_syndicate_loans(env: Env, syndicate_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::Syndicate(syndicate_id))
        .unwrap_or(Vec::new(&env))
}

/// #647: Create a new syndicate and return its ID.
/// Any caller may create a syndicate; loans are associated at request_loan time.
pub fn create_syndicate(env: Env) -> u64 {
    let id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::SyndicateCounter)
        .unwrap_or(0u64)
        + 1;
    env.storage()
        .persistent()
        .set(&DataKey::SyndicateCounter, &id);
    extend_ttl(&env, &DataKey::SyndicateCounter);
    // Initialise empty loan list so the key exists
    env.storage()
        .persistent()
        .set(&DataKey::Syndicate(id), &Vec::<u64>::new(&env));
    extend_ttl(&env, &DataKey::Syndicate(id));
    id
}

/// #646: Get the dynamic yield rate (bps) that would apply to a borrower right now.
pub fn get_dynamic_yield_bps(env: Env, borrower: Address) -> i128 {
    let cfg = config(&env);
    calculate_dynamic_yield(&env, &borrower, &cfg)
}

/// #646: Get the dynamic slash rate (bps) that would apply to a borrower right now.
pub fn get_dynamic_slash_bps(env: Env, borrower: Address) -> i128 {
    let cfg = config(&env);
    calculate_dynamic_slash(&env, &borrower, &cfg)
}

/// #646: Get the risk score for a borrower (0 = no risk, 10_000 = max risk).
pub fn get_risk_score(env: Env, borrower: Address) -> i128 {
    compute_risk_score(&env, &borrower)
}

// Task 2: Large Loan Multi-Signature - Require admin approval for large loans
pub fn request_large_loan(
    env: Env,
    borrower: Address,
    amount: i128,
    threshold: i128,
    loan_purpose: soroban_sdk::String,
    loan_category: LoanCategory,
    token_addr: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    // Check if loan amount exceeds large loan threshold
    let large_loan_threshold = crate::types::LARGE_LOAN_THRESHOLD;
    if amount <= large_loan_threshold {
        return Err(ContractError::LoanTooLarge);
    }

    // Check borrower doesn't already have an active loan
    assert!(
        !has_active_loan(&env, &borrower),
        "borrower already has an active loan"
    );

    // Validate token is allowed
    let _token_client = require_allowed_token(&env, &token_addr)?;

    // Store the large loan request
    let request_record = crate::types::LargeLoanRequestRecord {
        borrower: borrower.clone(),
        amount,
        requested_at: env.ledger().timestamp(),
        token_address: token_addr.clone(),
        threshold,
        loan_purpose: loan_purpose.clone(),
        loan_category,
    };

    env.storage()
        .persistent()
        .set(&DataKey::LargeLoanRequest(borrower.clone()), &request_record);
    extend_ttl(&env, &DataKey::LargeLoanRequest(borrower.clone()));

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("large_req")),
        (borrower, amount, token_addr),
    );

    Ok(())
}

pub fn approve_large_loan(
    env: Env,
    admin: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    admin.require_auth();

    // Verify admin is in the admin list
    let cfg = config(&env);
    let is_admin = cfg.admins.iter().any(|a| a == admin);
    assert!(is_admin, "caller is not an admin");

    // Get the large loan request
    let request: crate::types::LargeLoanRequestRecord = env
        .storage()
        .persistent()
        .get(&DataKey::LargeLoanRequest(borrower.clone()))
        .ok_or(ContractError::LargeLoanNotApproved)?;

    // Check if delay has elapsed
    let now = env.ledger().timestamp();
    let delay_elapsed = now >= request.requested_at + crate::types::LARGE_LOAN_DELAY_SECONDS;
    assert!(delay_elapsed, "large loan delay has not elapsed");

    // Update or create approval record
    let approval_key = DataKey::LargeLoanApproval(borrower.clone());
    let mut approval: crate::types::LargeLoanApprovalRecord = env
        .storage()
        .persistent()
        .get(&approval_key)
        .unwrap_or(crate::types::LargeLoanApprovalRecord {
            borrower: borrower.clone(),
            amount: request.amount,
            approved_by: Vec::new(&env),
            approval_timestamp: now,
            executed: false,
        });

    // Check admin hasn't already approved
    assert!(
        !approval.approved_by.iter().any(|a| a == admin),
        "admin already approved this loan"
    );

    // Add admin to approval list
    approval.approved_by.push_back(admin.clone());
    approval.approval_timestamp = now;

    env.storage()
        .persistent()
        .set(&approval_key, &approval);
    extend_ttl(&env, &approval_key);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("lg_appr")),
        (borrower, request.amount, admin),
    );

    Ok(())
}

pub fn execute_large_loan(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();

    // Get the large loan request
    let request: crate::types::LargeLoanRequestRecord = env
        .storage()
        .persistent()
        .get(&DataKey::LargeLoanRequest(borrower.clone()))
        .ok_or(ContractError::LargeLoanNotApproved)?;

    // Get the approval record
    let approval: crate::types::LargeLoanApprovalRecord = env
        .storage()
        .persistent()
        .get(&DataKey::LargeLoanApproval(borrower.clone()))
        .ok_or(ContractError::LargeLoanNotApproved)?;

    // Check if already executed
    assert!(!approval.executed, "large loan already executed");

    // Verify admin threshold is met
    let cfg = config(&env);
    assert!(
        approval.approved_by.len() as u32 >= cfg.admin_threshold,
        "insufficient admin approvals"
    );

    // Now execute the loan (similar to request_loan but for large loans)
    let now = env.ledger().timestamp();
    let deadline = now + cfg.loan_duration;
    let loan_id = next_loan_id(&env);
    let yield_bps = env
        .storage()
        .persistent()
        .get::<crate::types::DataKey, crate::types::TokenConfig>(
            &crate::types::DataKey::TokenConfig(request.token_address.clone()),
        )
        .map(|tc| tc.yield_bps)
        .unwrap_or(cfg.yield_bps);
    let total_yield = bps_of(request.amount, yield_bps);

    env.storage().persistent().set(
        &DataKey::Loan(loan_id),
        &LoanRecord {
            id: loan_id,
            borrower: borrower.clone(),
            co_borrowers: Vec::new(&env),
            amount: request.amount,
            amount_repaid: 0,
            total_yield,
            yield_bps,
            slash_bps: cfg.slash_bps,
            status: LoanStatus::Active,
            created_at: now,
            disbursement_timestamp: now,
            repayment_timestamp: None,
            deadline,
            loan_purpose: request.loan_purpose,
            loan_category: request.loan_category,
            token_address: request.token_address.clone(),
            syndicate_id: None,
        },
    );
    extend_ttl(&env, &DataKey::Loan(loan_id));
    env.storage()
        .persistent()
        .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
    extend_ttl(&env, &DataKey::ActiveLoan(borrower.clone()));
    env.storage()
        .persistent()
        .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);
    extend_ttl(&env, &DataKey::LatestLoan(borrower.clone()));

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::LoanCount(borrower.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::LoanCount(borrower.clone()), &(count + 1));
    extend_ttl(&env, &DataKey::LoanCount(borrower.clone()));

    // Disburse funds
    let token_client = soroban_sdk::token::Client::new(&env, &request.token_address);
    token_client.transfer(&env.current_contract_address(), &borrower, &request.amount);

    // Mark approval as executed
    let mut updated_approval = approval;
    updated_approval.executed = true;
    env.storage()
        .persistent()
        .set(&DataKey::LargeLoanApproval(borrower.clone()), &updated_approval);

    // Remove the request
    env.storage()
        .persistent()
        .remove(&DataKey::LargeLoanRequest(borrower.clone()));

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("lg_exec")),
        (borrower, request.amount),
    );

    Ok(())
}
