use soroban_sdk::{Env, String};
use crate::types::DataKey;

/// Metrics data structure
#[derive(Clone)]
pub struct Metrics {
    pub total_loans: u64,
    pub active_loans: u64,
    pub total_vouches: u64,
    pub total_stake: i128,
    pub total_yield_distributed: i128,
    pub total_slashed: i128,
    pub contract_balance: i128,
}

/// Get current metrics for monitoring
pub fn get_metrics(env: Env) -> Metrics {
    let total_loans = env.storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::LoanCount)
        .unwrap_or(0);
    
    let active_loans = env.storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::ActiveLoanCount)
        .unwrap_or(0);
    
    let total_vouches = env.storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::VouchCount)
        .unwrap_or(0);
    
    let total_stake = env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalStake)
        .unwrap_or(0);
    
    let total_yield_distributed = env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalYieldDistributed)
        .unwrap_or(0);
    
    let total_slashed = env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::TotalSlashed)
        .unwrap_or(0);
    
    let contract_balance = env.storage()
        .instance()
        .get::<DataKey, i128>(&DataKey::ContractBalance)
        .unwrap_or(0);
    
    Metrics {
        total_loans,
        active_loans,
        total_vouches,
        total_stake,
        total_yield_distributed,
        total_slashed,
        contract_balance,
    }
}

/// Format metrics as Prometheus-compatible output
pub fn format_prometheus_metrics(env: Env) -> String {
    let metrics = get_metrics(env);
    
    let mut output = String::new(&env);
    output.append(&String::from_slice(&env, "# HELP quorum_credit_total_loans Total number of loans\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_total_loans gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_total_loans {}\n", metrics.total_loans)));
    
    output.append(&String::from_slice(&env, "# HELP quorum_credit_active_loans Number of active loans\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_active_loans gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_active_loans {}\n", metrics.active_loans)));
    
    output.append(&String::from_slice(&env, "# HELP quorum_credit_total_vouches Total number of vouches\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_total_vouches gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_total_vouches {}\n", metrics.total_vouches)));
    
    output.append(&String::from_slice(&env, "# HELP quorum_credit_total_stake Total staked amount in stroops\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_total_stake gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_total_stake {}\n", metrics.total_stake)));
    
    output.append(&String::from_slice(&env, "# HELP quorum_credit_total_yield_distributed Total yield distributed in stroops\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_total_yield_distributed gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_total_yield_distributed {}\n", metrics.total_yield_distributed)));
    
    output.append(&String::from_slice(&env, "# HELP quorum_credit_total_slashed Total amount slashed in stroops\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_total_slashed gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_total_slashed {}\n", metrics.total_slashed)));
    
    output.append(&String::from_slice(&env, "# HELP quorum_credit_contract_balance Contract balance in stroops\n"));
    output.append(&String::from_slice(&env, "# TYPE quorum_credit_contract_balance gauge\n"));
    output.append(&String::from_slice(&env, &format!("quorum_credit_contract_balance {}\n", metrics.contract_balance)));
    
    output
}

/// Increment a metric counter
pub fn increment_metric(env: Env, key: DataKey, amount: u64) {
    let current = env.storage()
        .instance()
        .get::<DataKey, u64>(&key)
        .unwrap_or(0);
    
    env.storage()
        .instance()
        .set(&key, &(current + amount));
}

/// Increment a metric gauge (for i128 values)
pub fn increment_gauge(env: Env, key: DataKey, amount: i128) {
    let current = env.storage()
        .instance()
        .get::<DataKey, i128>(&key)
        .unwrap_or(0);
    
    env.storage()
        .instance()
        .set(&key, &(current + amount));
}
