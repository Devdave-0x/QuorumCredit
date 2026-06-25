use crate::errors::ContractError;
use crate::helpers::{
    config, deduct_slash_balance, get_active_loan_record, get_latest_loan_record,
    has_active_loan, next_loan_id, register_borrower_if_needed, require_allowed_token,
    require_not_paused, require_not_thawing, require_admin_approval,
};
use crate::reputation::ReputationNftExternalClient;
use crate::types::{
    DataKey, EscrowStatus, LoanRecord, LoanStatus, SlashRecord, VouchRecord, VoucherStats,
    BPS_DENOMINATOR, SLASH_ESCROW_PERIOD,
};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, Vec};

/// Vouch-age bonus constants
const VOUCH_AGE_BONUS_MIN_SECS: u64 = 30 * 24 * 60 * 60;   // 30 days
const VOUCH_AGE_BONUS_PERIOD_SECS: u64 = 30 * 24 * 60 * 60; // 30 days per period
const VOUCH_AGE_BONUS_BPS_PER_PERIOD: i128 = 25;             // +25 bps per period
const VOUCH_AGE_BONUS_MAX_BPS: i128 = 200;                   // cap at 200 bps

/// Compute the yield rate (in bps) for a single vouch, incorporating:
/// - base yield from config
/// - vouch-age bonus: +25 bps per 30-day period the vouch has been active (capped at 200 bps)
/// - borrower reputation bonus: up to +100 bps based on successful repayment history
/// - voucher reputation bonus: up to +100 bps based on voucher's successful vouch history (Issue #866)
pub fn vouch_yield_bps(env: &Env, vouch: &VouchRecord, borrower: &Address, now: u64) -> i128 {
    let base_bps = config(env).yield_bps;

    // ── Vouch-age bonus ───────────────────────────────────────────────────────
    let age_secs = now.saturating_sub(vouch.vouch_timestamp);
    let age_bonus = if age_secs >= VOUCH_AGE_BONUS_MIN_SECS {
        let periods = (age_secs / VOUCH_AGE_BONUS_PERIOD_SECS) as i128;
        (periods * VOUCH_AGE_BONUS_BPS_PER_PERIOD).min(VOUCH_AGE_BONUS_MAX_BPS)
    } else {
        0
    };

    // ── Borrower reputation bonus ─────────────────────────────────────────────
    // Successful repayments → higher bonus; defaults → penalty.
    let repayment_count: i128 = env
        .storage()
        .persistent()
        .get::<DataKey, u32>(&DataKey::RepaymentCount(borrower.clone()))
        .unwrap_or(0) as i128;
    let default_count: i128 = env
        .storage()
        .persistent()
        .get::<DataKey, u32>(&DataKey::DefaultCount(borrower.clone()))
        .unwrap_or(0) as i128;
    // +10 bps per successful repayment, -20 bps per default, capped at [0, REPUTATION_BONUS_MAX_BPS]
    let rep_bonus = ((repayment_count * 10) - (default_count * 20))
        .max(0)
        .min(REPUTATION_BONUS_MAX_BPS);

    // ── Voucher reputation bonus (Issue #866) ─────────────────────────────────
    // Vouchers with more successful vouch history earn a yield bonus.
    let voucher_stats: Option<VoucherStats> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherStats(vouch.voucher.clone()));
    let voucher_rep_bonus = voucher_stats
        .map(|s| (s.successful_vouches as i128 * 10).min(REPUTATION_BONUS_MAX_BPS))
        .unwrap_or(0);

    (base_bps + age_bonus + rep_bonus + voucher_rep_bonus).max(0)
}

/// Calculate dynamic yield (legacy — used for backward-compat; prefer vouch_yield_bps per vouch).
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
    require_not_thawing(&env)?;
    crate::helpers::check_rate_limit(&env, &borrower)?;
    crate::helpers::check_permission(&env, &borrower, |p| p.can_request_loan)?;
    register_borrower_if_needed(&env, &borrower);

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

    // Compute reputation-weighted total stake (Issue #866)
    let total_stake: i128 = vouches
        .iter()
        .filter(|v| v.token == token_addr)
        .map(|v| {
            let weight = crate::vouch::vouch_reputation_weight(&env, &v.voucher);
            v.stake * weight / BPS_DENOMINATOR
        })
        .sum();

    if total_stake < threshold {
        panic_with_error!(&env, ContractError::InsufficientFunds);
    }

    let now = env.ledger().timestamp();
    let loan_id = next_loan_id(&env);

    // ── Credit score tier rewards ────────────────────────────────────────────
    // Apply tier-based yield bonus to base yield rate (Issue #866)
    let tier_adjusted_yield_bps = crate::credit_score::apply_tier_rewards_to_yield(
        &env, &borrower, config(&env).yield_bps,
    );

    // ── Per-vouch yield (age + reputation + tier + weight aware) ────────────
    // Compute each voucher's individual yield share using reputation-weighted stake.
    let mut yield_distribution: Vec<YieldDistributionEntry> = Vec::new(&env);
    let mut total_yield: i128 = 0;

    for v in vouches.iter() {
        if v.token != token_addr {
            continue;
        }
        let rate = vouch_yield_bps(&env, &v, &borrower, now);
        // Apply tier rewards on top of per-vouch rate (Issue #866)
        let effective_rate = rate + (tier_adjusted_yield_bps - config(&env).yield_bps);
        let weight = crate::vouch::vouch_reputation_weight(&env, &v.voucher);
        let weighted_stake = v.stake * weight / BPS_DENOMINATOR;
        let vouch_yield = amount * weighted_stake / total_stake * effective_rate / 10_000;
        total_yield += vouch_yield;
        yield_distribution.push_back(YieldDistributionEntry {
            voucher: v.voucher.clone(),
            yield_amount: vouch_yield,
        });
    }

    // Store yield distribution for repayment-time lookup
    env.storage()
        .persistent()
        .set(&DataKey::YieldDistribution(loan_id), &yield_distribution);

    let loan = LoanRecord {
        id: loan_id,
        borrower: borrower.clone(),
        guarantor: None,
        buyback_price: 0,
        auto_repay_enabled: false,
        auto_repay_attempts: 0,
        escrow_status: EscrowStatus::None,
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
        rate_type: crate::types::RateType::Fixed,
        index_reference: None,
        escrow_status: EscrowStatus::None,
        retry_count: 0,
    };

    env.storage().persistent().set(&DataKey::Loan(loan_id), &loan);
    env.storage()
        .persistent()
        .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
    env.storage()
        .persistent()
        .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

    token.transfer(&env.current_contract_address(), &borrower, &amount);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("created")),
        (borrower, amount),
    );

    Ok(())
}

/// Apply slash recovery to borrower when a defaulted loan is fully repaid.
pub fn apply_slash_recovery(env: &Env, borrower: &Address) -> Result<(), ContractError> {
    let cfg = config(env);
    if cfg.recovery_percentage == 0 {
        return Ok(());
    }

    let mut record: SlashRecord = match env
        .storage()
        .persistent()
        .get::<DataKey, SlashRecord>(&DataKey::SlashAudit(borrower.clone()))
    {
        Some(r) if !r.reversed => r,
        _ => return Ok(()),
    };

    if record.recovery_amount > 0 {
        return Ok(());
    }

    let recoverable = record.total_slashed * cfg.recovery_percentage as i128 / BPS_DENOMINATOR;
    if recoverable <= 0 {
        return Ok(());
    }

    deduct_slash_balance(env, recoverable)?;

    let loan = get_latest_loan_record(env, borrower).ok_or(ContractError::NoActiveLoan)?;
    let token = require_allowed_token(env, &loan.token_address)?;
    token.transfer(
        &env.current_contract_address(),
        borrower,
        &recoverable,
    );

    record.recovery_amount = recoverable;
    env.storage()
        .persistent()
        .set(&DataKey::SlashRecord(record.slash_id), &record);
    env.storage()
        .persistent()
        .set(&DataKey::SlashAudit(borrower.clone()), &record);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("recovery")),
        (borrower.clone(), recoverable),
    );

    Ok(())
}

/// Repay loan (active or defaulted).
pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_thawing(&env)?;
    crate::helpers::check_rate_limit(&env, &borrower)?;
    crate::helpers::check_permission(&env, &borrower, |p| p.can_repay)?;

     let mut loan = match get_active_loan_record(&env, &borrower) {
         Ok(l) => l,
         Err(ContractError::NoActiveLoan) => {
             let l = get_latest_loan_record(&env, &borrower).ok_or(ContractError::NoActiveLoan)?;
             if l.status != LoanStatus::Defaulted {
                 env.events().publish(
                     (symbol_short!("loan"), symbol_short!("repayment_failure")),
                     (borrower.clone(), payment),
                 );
                 return Err(ContractError::NoActiveLoan);
             }
             l
         }
         Err(e) => {
             env.events().publish(
                 (symbol_short!("loan"), symbol_short!("repayment_failure")),
                 (borrower.clone(), payment),
             );
             return Err(e);
         }
     };

    let was_defaulted = loan.status == LoanStatus::Defaulted;

    if payment <= 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let total_owed = loan.amount.checked_add(loan.total_yield).ok_or(ContractError::ArithmeticError)?;
    let outstanding = total_owed.checked_sub(loan.amount_repaid).ok_or(ContractError::ArithmeticError)?;

    if payment > outstanding {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let token = require_allowed_token(&env, &loan.token_address)?;
    token.transfer(&borrower, &env.current_contract_address(), &payment);

    loan.amount_repaid = loan.amount_repaid.checked_add(payment).ok_or(ContractError::ArithmeticError)?;

    let now = env.ledger().timestamp();
    let cfg = config(&env);

    let mut penalty: i128 = 0;
    if !was_defaulted && now < loan.deadline && cfg.prepayment_penalty_bps > 0 {
        let repaid_principal = loan.amount_repaid.checked_mul(loan.amount).ok_or(ContractError::ArithmeticError)? / total_owed;
        let remaining_principal = loan.amount.checked_sub(repaid_principal).ok_or(ContractError::ArithmeticError)?;
        penalty = remaining_principal.checked_mul(cfg.prepayment_penalty_bps as i128).ok_or(ContractError::ArithmeticError)? / BPS_DENOMINATOR;
    }

    let fully_repaid = loan.amount_repaid >= total_owed;

    if fully_repaid && was_defaulted {
        loan.status = LoanStatus::Repaid;
        loan.repayment_timestamp = Some(now);
        apply_slash_recovery(&env, &borrower)?;
        env.events().publish(
            (symbol_short!("loan"), symbol_short!("repaid")),
            (borrower.clone(), loan.amount),
        );
    } else if fully_repaid {
        loan.status = LoanStatus::Repaid;
        loan.repayment_timestamp = Some(now);

        // Process withdrawal queue BEFORE vouch payout (Issue #865: progressive stake unlock).
        // This removes queued withdrawals from the vouches list and transfers their stake back.
        crate::vouch::process_withdrawal_queue(&env, &borrower);

        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        // Load the per-vouch yield distribution locked in at disbursement.
        let yield_dist: Vec<YieldDistributionEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::YieldDistribution(loan.id))
            .unwrap_or(Vec::new(&env));

        // Any penalty is distributed proportionally to stake (fallback).
        let total_stake: i128 = vouches
            .iter()
            .filter(|v| v.token == loan.token_address)
            .map(|v| v.stake)
            .sum();

        for v in vouches.iter() {
            if v.token != loan.token_address {
                continue;
            }

            // Per-vouch yield from the distribution locked at disbursement.
            let vouch_yield = yield_dist
                .iter()
                .find(|e| e.voucher == v.voucher)
                .map(|e| e.yield_amount)
                .unwrap_or(0);

            // Penalty share is proportional to stake.
            let penalty_share = if total_stake > 0 {
                penalty * v.stake / total_stake
            } else {
                0
            };

            let payout = v.stake + vouch_yield + penalty_share;
            token.transfer(&env.current_contract_address(), &v.voucher, &payout);

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
            stats.total_yield_earned += vouch_yield;
            env.storage()
                .persistent()
                .set(&DataKey::VoucherStats(v.voucher.clone()), &stats);
        }

        // Increment borrower repayment count (feeds future reputation bonus).
        let prev_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::RepaymentCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::RepaymentCount(borrower.clone()), &(prev_count + 1));

        // Try to mint excellent credit tier badge if eligible
        let _ = crate::reputation::mint_excellent_badge(&env, &borrower);

        // Update credit score after successful repayment
        let _ = crate::credit_score::update_credit_score(env.clone(), borrower.clone());

        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::YieldDistribution(loan.id));

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
        .map(|v| {
            let weight = crate::vouch::vouch_reputation_weight(&env, &v.voucher);
            v.stake * weight / BPS_DENOMINATOR
        })
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
    require_not_thawing(&env)?;
    crate::helpers::check_rate_limit(&env, &borrower)?;
    crate::helpers::check_permission(&env, &borrower, |p| p.can_repay)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if payment <= 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let token_client = require_allowed_token(&env, &token)?;

    // FIX: transfer should be FROM borrower TO contract
    token_client.transfer(&borrower, &env.current_contract_address(), &payment);

    loan.amount_repaid = loan.amount_repaid.checked_add(payment).ok_or(ContractError::ArithmeticError)?;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("prt_rep")),
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
    require_admin_approval(&env, &admins);

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
    require_admin_approval(&env, &admins);

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

pub fn release_slash_escrow(
    env: Env,
    admin_signers: Vec<Address>,
    borrower: Address,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    let (amount, release_ts): (i128, u64) = env
        .storage()
        .persistent()
        .get(&DataKey::SlashEscrow(borrower.clone()))
        .ok_or(ContractError::NoActiveLoan)?;
    if env.ledger().timestamp() < release_ts {
        return Err(ContractError::TimelockNotReady);
    }
    crate::helpers::add_slash_balance(&env, amount);
    env.storage()
        .persistent()
        .remove(&DataKey::SlashEscrow(borrower));
    Ok(())
}

pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
    if let Ok(loan) = get_active_loan_record(&env, &borrower) {
        return Some(loan);
    }
    get_latest_loan_record(&env, &borrower)
}

pub fn get_loan_by_id(env: Env, loan_id: u64) -> Option<LoanRecord> {
    env.storage().persistent().get(&DataKey::Loan(loan_id))
}

pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
    get_loan(env, borrower)
        .map(|l| l.status)
        .unwrap_or(LoanStatus::None)
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

pub fn register_referral(
    _env: Env,
    _borrower: Address,
    _referrer: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn get_referrer(_env: Env, _borrower: Address) -> Option<Address> {
    None
}

pub fn add_co_borrower(
    _env: Env,
    _borrower: Address,
    _co_borrower: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn refinance_loan(
    _env: Env,
    _borrower: Address,
    _new_amount: i128,
    _new_threshold: i128,
    _new_token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn deposit_collateral(
    _env: Env,
    _borrower: Address,
    _amount: i128,
    _token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn get_borrower_collateral(_env: Env, _borrower: Address) -> i128 {
    0
}

pub fn emit_repayment_reminders(_env: Env) {}
pub fn mint_reputation_nft(_env: Env, _borrower: Address) -> Result<(), ContractError> {
    Ok(())
}
pub fn send_repayment_reminder(_env: Env, _loan_id: u64) -> Result<(), ContractError> {
    Ok(())
}

pub fn request_extension(
    _env: Env,
    _borrower: Address,
    _extension_secs: u64,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn approve_extension(
    _env: Env,
    _voucher: Address,
    _borrower: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn get_extension_request(
    _env: Env,
    _borrower: Address,
) -> Option<crate::types::LoanExtensionRequest> {
    None
}

pub fn defer_payment(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_thawing(&env)?;
    Err(ContractError::InvalidStateTransition)
}

pub fn check_acceleration(env: Env, _borrower: Address) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn set_maturity_date(
    env: Env,
    admin_signers: Vec<Address>,
    borrower: Address,
    _maturity_date: u64,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    let mut loan = get_active_loan_record(&env, &borrower)?;
    loan.maturity_date = Some(_maturity_date);
    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);
    Ok(())
}

pub fn set_loan_rate(
    env: Env,
    admin_signers: Vec<Address>,
    borrower: Address,
    rate_type: crate::types::RateType,
    index_reference: Option<soroban_sdk::String>,
) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    let mut loan = get_active_loan_record(&env, &borrower)?;
    loan.rate_type = rate_type;
    loan.index_reference = index_reference;
    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);
    Ok(())
}

// ── Issue #656: Loan Guarantee ────────────────────────────────────────────────

pub fn set_loan_guarantor(
    env: Env,
    borrower: Address,
    guarantor: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_thawing(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if loan.guarantor.is_some() {
        return Err(ContractError::InvalidStateTransition);
    }

    loan.guarantor = Some(guarantor);

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("guarantor")),
        (borrower, guarantor),
    );

    Ok(())
}

pub fn remove_loan_guarantor(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_thawing(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;
    loan.guarantor = None;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    Ok(())
}

// ── Issue #657: Loan Buyback ────────────────────────────────────────────────

pub fn set_buyback_price(
    env: Env,
    borrower: Address,
    price: i128,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_thawing(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if price < 0 {
        return Err(ContractError::InvalidAmount);
    }

    loan.buyback_price = price;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("buyback_set")),
        (borrower, price),
    );

    Ok(())
}

pub fn buyback_loan(
    env: Env,
    voucher: Address,
    borrower: Address,
    amount: i128,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;

    let loan = get_active_loan_record(&env, &borrower)?;

    if loan.buyback_price == 0 {
        return Err(ContractError::InvalidStateTransition);
    }

    let total_stake: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env))
        .iter()
        .filter(|v| v.voucher == voucher && v.token == loan.token_address)
        .map(|v| v.stake)
        .sum();

    if total_stake == 0 {
        return Err(ContractError::VoucherNotFound);
    }

    let cost = total_stake * loan.buyback_price / BPS_DENOMINATOR;
    if amount < cost {
        return Err(ContractError::InvalidAmount);
    }

    let token_client = require_allowed_token(&env, &loan.token_address)?;
    token_client.transfer(&voucher, &env.current_contract_address(), &cost);
    token_client.transfer(&env.current_contract_address(), &voucher, &total_stake);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("buyback")),
        (borrower, voucher, cost, total_stake),
    );

    Ok(())
}

// ── Issue #658: Automatic Repayment ───────────────────────────────────────────

pub fn enable_auto_repay(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    if loan.auto_repay_enabled {
        return Err(ContractError::InvalidStateTransition);
    }

    loan.auto_repay_enabled = true;
    loan.auto_repay_attempts = 0;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("auto_repay_on")),
        borrower,
    );

    Ok(())
}

pub fn disable_auto_repay(env: Env, borrower: Address) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;
    loan.auto_repay_enabled = false;

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("auto_repay_off")),
        borrower,
    );

    Ok(())
}

