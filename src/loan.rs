use crate::errors::ContractError;
use crate::helpers::{
    config, get_active_loan_record, has_active_loan, next_loan_id, require_allowed_token,
    require_not_paused, require_admin_approval,
};
use crate::reputation::ReputationNftExternalClient;
use crate::types::{
    DataKey, LoanRecord, LoanStatus, VouchRecord, BPS_DENOMINATOR,
    DEFAULT_REFERRAL_BONUS_BPS, SLASH_ESCROW_PERIOD, MAX_DEFERMENT_PERIODS, DEFERMENT_PERIOD_SECS,
};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, Vec};

/// Calculate dynamic yield
pub fn calculate_dynamic_yield(env: &Env, borrower: &Address) -> i128 {
    let base_bps = config(env).yield_bps;

    let credit_score: i128 = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::ReputationNft)
        .map(|nft| ReputationNftExternalClient::new(env, &nft).balance(borrower) as i128)
        .unwrap_or(0);

    let default_count: i128 = env
        .storage()
        .persistent()
        .get::<DataKey, u32>(&DataKey::DefaultCount(borrower.clone()))
        .unwrap_or(0) as i128;

    (base_bps + (credit_score / 100) - (default_count * 50)).max(0)
}

/// Request loan
pub fn request_loan(
    env: Env,
    borrower: Address,
    amount: i128,
    threshold: i128,
    loan_purpose: soroban_sdk::String,
    token_addr: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    if has_active_loan(&env, &borrower) {
        return Err(ContractError::ActiveLoanExists);
    }

    let token = require_allowed_token(&env, &token_addr)?;
    let cfg = config(&env);

    if amount < cfg.min_loan_amount {
        return Err(ContractError::LoanBelowMinAmount);
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    let total_stake: i128 = vouches
        .iter()
        .filter(|v| v.token == token_addr)
        .map(|v| v.stake)
        .sum();

    if total_stake < threshold {
        panic_with_error!(&env, ContractError::InsufficientFunds);
    }

    let now = env.ledger().timestamp();
    let loan_id = next_loan_id(&env);

    let yield_bps = calculate_dynamic_yield(&env, &borrower);
    let total_yield = amount * yield_bps / 10_000;

    let loan = LoanRecord {
        id: loan_id,
        borrower: borrower.clone(),
        co_borrowers: Vec::new(&env),
        amount,
        amount_repaid: 0,
        total_yield,
        status: LoanStatus::Active,
        created_at: now,
        disbursement_timestamp: now,
        repayment_timestamp: None,
        deadline: now + cfg.loan_duration,
        loan_purpose,
        token_address: token_addr.clone(),
        amortization_schedule: Vec::new(&env),
        reminder_sent: false,
        risk_score: 0,
        deferment_periods: 0,
        maturity_date: None,
    };

    env.storage().persistent().set(&DataKey::Loan(loan_id), &loan);
    env.storage().persistent().set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);

    token.transfer(&env.current_contract_address(), &borrower, &amount);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("created")),
        (borrower, amount),
    );

    Ok(())
}

/// Repay loan (FULL UPDATED VERSION)
pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if payment <= 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let total_owed = loan.amount + loan.total_yield;
    let outstanding = total_owed - loan.amount_repaid;

    if payment > outstanding {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let token = require_allowed_token(&env, &loan.token_address)?;
    token.transfer(&borrower, &env.current_contract_address(), &payment);

    loan.amount_repaid += payment;

    let now = env.ledger().timestamp();
    let cfg = config(&env);

    // PREPAYMENT PENALTY (FIXED + SAFE)
    let mut penalty: i128 = 0;
    if now < loan.deadline && cfg.prepayment_penalty_bps > 0 {
        let remaining_principal =
            loan.amount - (loan.amount_repaid * loan.amount / total_owed);
        penalty = remaining_principal * cfg.prepayment_penalty_bps as i128 / 10_000;
    }

    if loan.amount_repaid >= total_owed {
        loan.status = LoanStatus::Repaid;
        loan.repayment_timestamp = Some(now);

        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let total_stake: i128 = vouches
            .iter()
            .filter(|v| v.token == loan.token_address)
            .map(|v| v.stake)
            .sum();

        let total_yield_pool = loan.total_yield + penalty;

        for v in vouches.iter() {
            if v.token != loan.token_address {
                continue;
            }

            let share = if total_stake > 0 {
                total_yield_pool * v.stake / total_stake
            } else {
                0
            };

            token.transfer(
                &env.current_contract_address(),
                &v.voucher,
                &(v.stake + share),
            );

            // Issue #602: Update voucher reputation stats on successful repayment.
            let mut stats: crate::types::VoucherStats = env
                .storage()
                .persistent()
                .get(&DataKey::VoucherStats(v.voucher.clone()))
                .unwrap_or(crate::types::VoucherStats {
                    successful_vouches: 0,
                    total_vouches_slashed: 0,
                    total_yield_earned: 0,
                    total_slashed: 0,
                });
            stats.successful_vouches += 1;
            stats.total_yield_earned += voucher_yield;
            env.storage()
                .persistent()
                .set(&DataKey::VoucherStats(v.voucher.clone()), &stats);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("repaid")),
            (borrower.clone(), loan.amount),
        );
    }

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    Ok(())
}

/// Eligibility check
pub fn is_eligible(env: Env, borrower: Address, threshold: i128, token: Address) -> bool {
    if threshold <= 0 {
        return false;
    }

    if has_active_loan(&env, &borrower) {
        return false;
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));

    let total: i128 = vouches
        .iter()
        .filter(|v| v.token == token)
        .map(|v| v.stake)
        .sum();

    total >= threshold
}

/// Partial repay (FIXED DIRECTION BUG)
pub fn repay_partial(
    env: Env,
    borrower: Address,
    payment: i128,
    token: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if payment <= 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let token_client = require_allowed_token(&env, &token)?;

    // FIX: transfer should be FROM borrower TO contract
    token_client.transfer(&borrower, &env.current_contract_address(), &payment);

    loan.amount_repaid += payment;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("partial_repay")),
        (borrower, payment),
    );

    Ok(())
}

/// Set yield reserve
pub fn set_yield_reserve(
    env: Env,
    admins: Vec<Address>,
    amount: i128,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admins)?;

    if amount < 0 {
        return Err(ContractError::InvalidAmount);
    }

    env.storage()
        .persistent()
        .set(&DataKey::YieldReserve, &amount);

    Ok(())
}

/// Get yield reserve
pub fn get_yield_reserve_balance(env: Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::YieldReserve)
        .unwrap_or(0)
}

/// Set borrower risk score
pub fn set_borrower_risk_score(
    env: Env,
    admins: Vec<Address>,
    borrower: Address,
    risk_score: u32,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admins)?;

    if risk_score > 100 {
        return Err(ContractError::InvalidAmount);
    }

    let mut loan = get_active_loan_record(&env, &borrower)?;
    loan.risk_score = risk_score;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    Ok(())
}

/// Defer the next payment, extending the loan deadline by one deferment period.
/// Limited to `MAX_DEFERMENT_PERIODS` per loan.
pub fn defer_payment(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if loan.deferment_periods >= MAX_DEFERMENT_PERIODS {
        return Err(ContractError::DefermentLimitReached);
    }

    loan.deadline += DEFERMENT_PERIOD_SECS;
    loan.deferment_periods += 1;

    env.storage().persistent().set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!(\"loan\"), symbol_short!(\"deferred\")),
        (borrower, loan.deferment_periods),
    );

    Ok(())
}

/// Check whether a borrower's loan should be accelerated based on their default count
/// against the configured `acceleration_triggers`. If any trigger threshold is met,
/// the loan deadline is set to the current timestamp (immediately due) and an event
/// is emitted. Returns `LoanAccelerated` if triggered.
pub fn check_acceleration(env: Env, borrower: Address) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    let cfg = config(&env);
    if cfg.acceleration_triggers.is_empty() {
        return Ok(());
    }

    let default_count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::DefaultCount(borrower.clone()))
        .unwrap_or(0);

    let triggered = cfg
        .acceleration_triggers
        .iter()
        .any(|threshold| default_count >= threshold);

    if !triggered {
        return Ok(());
    }

    let mut loan = get_active_loan_record(&env, &borrower)?;
    let now = env.ledger().timestamp();
    loan.deadline = now;

    env.storage().persistent().set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!(\"loan\"), symbol_short!(\"accel\")),
        (borrower, default_count),
    );

    Err(ContractError::LoanAccelerated)
}

/// Set a custom maturity date for an active loan. Admin-only.
/// The maturity_date must be in the future and after the current deadline.
pub fn set_maturity_date(
    env: Env,
    admin_signers: Vec<Address>,
    borrower: Address,
    maturity_date: u64,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers)?;

    let now = env.ledger().timestamp();
    if maturity_date <= now {
        return Err(ContractError::InvalidAmount);
    }

    let mut loan = get_active_loan_record(&env, &borrower)?;
    loan.maturity_date = Some(maturity_date);

    env.storage().persistent().set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!(\"loan\"), symbol_short!(\"maturity\")),
        (borrower, maturity_date),
    );

    Ok(())
}
