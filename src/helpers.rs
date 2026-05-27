use crate::errors::ContractError;
use crate::types::{Config, DataKey, LoanRecord};
use soroban_sdk::{token, Address, Env};

pub fn require_not_paused(env: &Env) -> Result<(), ContractError> {
    let paused: bool = env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);
    if paused {
        Err(ContractError::ContractPaused)
    } else {
        Ok(())
    }
}

/// Returns `Err(InsufficientFunds)` if `amount` is not strictly positive (≤ 0).
/// Use this for all numeric inputs that must be > 0 (stakes, loan amounts, thresholds).
/// All such amounts are denominated in stroops (1 XLM = 10,000,000 stroops).
pub fn require_positive_amount(_env: &Env, amount: i128) -> Result<(), ContractError> {
    if amount <= 0 {
        return Err(ContractError::InsufficientFunds);
    }
    Ok(())
}

pub fn config(env: &Env) -> Config {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .expect("not initialized")
}

pub fn has_active_loan(env: &Env, borrower: &Address) -> bool {
    matches!(get_active_loan_record(env, borrower), Ok(loan) if loan.status == crate::types::LoanStatus::Active)
}

pub fn get_active_loan_record(env: &Env, borrower: &Address) -> Result<LoanRecord, ContractError> {
    let loan_id: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::ActiveLoan(borrower.clone()))
        .ok_or(ContractError::NoActiveLoan)?;
    env.storage()
        .persistent()
        .get(&DataKey::Loan(loan_id))
        .ok_or(ContractError::NoActiveLoan)
}

/// Returns a token client for `addr` after verifying it is an allowed token
/// (either the primary protocol token or in `Config.allowed_tokens`).
pub fn require_allowed_token<'a>(
    env: &'a Env,
    addr: &Address,
) -> Result<token::Client<'a>, ContractError> {
    let cfg = config(env);
    if *addr == cfg.token || cfg.allowed_tokens.iter().any(|t| t == *addr) {
        Ok(token::Client::new(env, addr))
    } else {
        Err(ContractError::InvalidToken)
    }
}

/// Calculate dynamic slash threshold based on protocol health.
/// Returns the effective slash_bps to use for slashing operations.
/// 
/// When dynamic_slash_threshold is enabled:
/// - Healthy protocol (≥80% health): Lower slash penalty (25-50%)
/// - Unhealthy protocol (<80% health): Higher slash penalty (50-75%)
/// 
/// Health is calculated based on:
/// - Yield reserve solvency (contract token balance)
/// - Contract initialization status
/// - Pause state
pub fn calculate_dynamic_slash_threshold(env: &Env) -> i128 {
    use crate::types::{MIN_DYNAMIC_SLASH_BPS, MAX_DYNAMIC_SLASH_BPS, HEALTH_THRESHOLD_BPS, BPS_DENOMINATOR};
    
    let cfg = config(env);
    
    // If dynamic threshold is disabled, return static value
    if !cfg.dynamic_slash_threshold {
        return cfg.slash_bps;
    }
    
    // Calculate protocol health score (0-10000 basis points)
    let health_score = calculate_protocol_health_score(env);
    
    // If health is above threshold (80%), use lower slash penalty
    if health_score >= HEALTH_THRESHOLD_BPS {
        // Interpolate between MIN and static slash_bps based on health
        // At 100% health: MIN_DYNAMIC_SLASH_BPS (25%)
        // At 80% health: cfg.slash_bps (50% default)
        let health_factor = (health_score - HEALTH_THRESHOLD_BPS) * BPS_DENOMINATOR / (BPS_DENOMINATOR - HEALTH_THRESHOLD_BPS);
        cfg.slash_bps - ((cfg.slash_bps - MIN_DYNAMIC_SLASH_BPS) * health_factor / BPS_DENOMINATOR)
    } else {
        // Health below threshold: increase slash penalty
        // At 80% health: cfg.slash_bps (50% default)
        // At 0% health: MAX_DYNAMIC_SLASH_BPS (75%)
        let health_factor = health_score * BPS_DENOMINATOR / HEALTH_THRESHOLD_BPS;
        cfg.slash_bps + ((MAX_DYNAMIC_SLASH_BPS - cfg.slash_bps) * (BPS_DENOMINATOR - health_factor) / BPS_DENOMINATOR)
    }
}

/// Calculate protocol health score as basis points (0-10000).
/// Considers multiple factors:
/// - Yield reserve solvency (40% weight)
/// - Contract initialization (30% weight) 
/// - Pause state (30% weight)
fn calculate_protocol_health_score(env: &Env) -> i128 {
    use crate::types::BPS_DENOMINATOR;
    
    let mut score = 0i128;
    
    // Check initialization (30% weight = 3000 bps)
    if env.storage().instance().has(&DataKey::Config) {
        score += 3_000;
    }
    
    // Check pause state (30% weight = 3000 bps)
    let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
    if !paused {
        score += 3_000;
    }
    
    // Check yield reserve solvency (40% weight = 4000 bps)
    if env.storage().instance().has(&DataKey::Config) {
        let cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        let token_client = token::Client::new(env, &cfg.token);
        let contract_balance = token_client.balance(&env.current_contract_address());
        
        // Scale solvency score based on balance relative to minimum threshold
        // Minimum threshold: 10 XLM (100_000_000 stroops)
        // Excellent threshold: 100 XLM (1_000_000_000 stroops)
        let min_threshold = 100_000_000i128; // 10 XLM
        let excellent_threshold = 1_000_000_000i128; // 100 XLM
        
        if contract_balance >= excellent_threshold {
            score += 4_000; // Full solvency score
        } else if contract_balance >= min_threshold {
            // Linear interpolation between min and excellent thresholds
            let solvency_factor = (contract_balance - min_threshold) * 4_000 / (excellent_threshold - min_threshold);
            score += 2_000 + solvency_factor; // Base 2000 + up to 2000 more
        } else if contract_balance > 0 {
            // Partial score for any balance above 0
            let solvency_factor = contract_balance * 2_000 / min_threshold;
            score += solvency_factor;
        }
        // If balance is 0, add nothing to score
    }
    
    // Ensure score is within bounds
    if score > BPS_DENOMINATOR {
        BPS_DENOMINATOR
    } else if score < 0 {
        0
    } else {
        score
    }
}
