/// #637: Vouch Fraud Detection
///
/// Detects suspicious vouch patterns such as a voucher backing many defaulted borrowers.
/// Computes a fraud_score (0–100) and stores it per voucher.
use soroban_sdk::{symbol_short, Address, Env, Vec};

use crate::{
    errors::ContractError,
    types::{
        DataKey, LoanStatus, VouchRecord, FRAUD_SCORE_CONCENTRATION_WEIGHT,
        FRAUD_SCORE_DEFAULT_WEIGHT, FRAUD_SCORE_HIGH_THRESHOLD, FRAUD_SCORE_MAX,
    },
};

/// Recalculate and store the fraud score for a voucher.
///
/// Score components:
/// - +20 per defaulted borrower backed (capped at 60)
/// - +10 if voucher backs more than 10 borrowers simultaneously (concentration risk)
///
/// Returns the new fraud score.
pub fn calculate_fraud_score(env: Env, voucher: Address) -> u32 {
    let history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher.clone()))
        .unwrap_or_else(|| Vec::new(&env));

    let total_backed = history.len();
    let mut default_count: u32 = 0;

    for borrower in history.iter() {
        // Check if this borrower has a defaulted loan
        if let Some(loan_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::ActiveLoan(borrower.clone()))
        {
            if let Some(loan) = env
                .storage()
                .persistent()
                .get::<DataKey, crate::types::LoanRecord>(&DataKey::Loan(loan_id))
            {
                if loan.status == LoanStatus::Defaulted {
                    default_count += 1;
                }
            }
        }
        // Also check LatestLoan for closed defaulted loans
        if let Some(loan_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::LatestLoan(borrower.clone()))
        {
            if let Some(loan) = env
                .storage()
                .persistent()
                .get::<DataKey, crate::types::LoanRecord>(&DataKey::Loan(loan_id))
            {
                if loan.status == LoanStatus::Defaulted {
                    default_count += 1;
                }
            }
        }
    }

    // Deduplicate: cap default contribution
    let default_score = (default_count * FRAUD_SCORE_DEFAULT_WEIGHT).min(60);

    // Concentration risk: backing more than 10 borrowers
    let concentration_score = if total_backed > 10 {
        FRAUD_SCORE_CONCENTRATION_WEIGHT
    } else {
        0
    };

    let score = (default_score + concentration_score).min(FRAUD_SCORE_MAX);

    env.storage()
        .persistent()
        .set(&DataKey::VoucherFraudScore(voucher.clone()), &score);

    if score >= FRAUD_SCORE_HIGH_THRESHOLD {
        env.events()
            .publish((symbol_short!("fraud_hi"),), (voucher, score));
    }

    score
}

/// Get the stored fraud score for a voucher (0 if never calculated).
pub fn get_fraud_score(env: Env, voucher: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::VoucherFraudScore(voucher))
        .unwrap_or(0)
}

/// Returns true if the voucher's fraud score is at or above the high threshold.
pub fn is_high_fraud_risk(env: Env, voucher: Address) -> bool {
    get_fraud_score(env, voucher) >= FRAUD_SCORE_HIGH_THRESHOLD
}
