/// #637: Vouch Fraud Detection
use soroban_sdk::{symbol_short, Address, Env, Vec};

use crate::types::{
    DataKey, LoanRecord, LoanStatus, FRAUD_SCORE_CONCENTRATION_WEIGHT,
    FRAUD_SCORE_DEFAULT_WEIGHT, FRAUD_SCORE_HIGH_THRESHOLD, FRAUD_SCORE_MAX,
};

pub fn calculate_fraud_score(env: Env, voucher: Address) -> u32 {
    let history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher.clone()))
        .unwrap_or_else(|| Vec::new(&env));

    let total_backed = history.len();
    let mut default_count: u32 = 0;

    for borrower in history.iter() {
        // Check latest loan for this borrower
        if let Some(loan_id) = env
            .storage()
            .persistent()
            .get::<DataKey, u64>(&DataKey::LatestLoan(borrower.clone()))
        {
            if let Some(loan) = env
                .storage()
                .persistent()
                .get::<DataKey, LoanRecord>(&DataKey::Loan(loan_id))
            {
                if loan.status == LoanStatus::Defaulted {
                    default_count += 1;
                }
            }
        }
    }

    let default_score = (default_count * FRAUD_SCORE_DEFAULT_WEIGHT).min(60);
    let concentration_score = if total_backed > 10 { FRAUD_SCORE_CONCENTRATION_WEIGHT } else { 0 };
    let score = (default_score + concentration_score).min(FRAUD_SCORE_MAX);

    env.storage()
        .persistent()
        .set(&DataKey::VoucherFraudScore(voucher.clone()), &score);

    if score >= FRAUD_SCORE_HIGH_THRESHOLD {
        env.events().publish((symbol_short!("fraud"), symbol_short!("hi")), (voucher, score));
    }

    score
}

pub fn get_fraud_score(env: Env, voucher: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::VoucherFraudScore(voucher))
        .unwrap_or(0)
}

pub fn is_high_fraud_risk(env: Env, voucher: Address) -> bool {
    get_fraud_score(env, voucher) >= FRAUD_SCORE_HIGH_THRESHOLD
}
