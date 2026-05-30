#![allow(unused_imports)]
use crate::errors::ContractError;
use crate::helpers::{
    config, extend_ttl, require_admin_approval, require_valid_token, validate_admin_config,
};
use crate::types::{
    AdminAuditEntry, AdminTimelock, AdminTimelockAction, Config, DataKey, PauseFlag, TokenConfig,
    VoucherStakeLimitKey,
};
use soroban_sdk::{symbol_short, Address, BytesN, Env, String, Vec};

// ── Internal helpers ──────────────────────────────────────────────────────────

fn log_admin_action(env: &Env, admin: &Address, action: &str) {
    let mut log: Vec<AdminAuditEntry> = env
        .storage()
        .instance()
        .get(&DataKey::AdminAuditLog)
        .unwrap_or(Vec::new(env));
    log.push_back(AdminAuditEntry {
        admin: admin.clone(),
        action: String::from_str(env, action),
        timestamp: env.ledger().timestamp(),
    });
    env.storage().instance().set(&DataKey::AdminAuditLog, &log);
}

pub fn is_admin_key_expired(env: &Env, admin: &Address) -> bool {
    let expiry: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AdminKeyExpiry(admin.clone()))
        .unwrap_or(0);
    expiry > 0 && env.ledger().timestamp() > expiry
}

// ── Admin management ──────────────────────────────────────────────────────────

pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    assert!(!cfg.admins.iter().any(|a| a == new_admin), "already an admin");
    cfg.admins.push_back(new_admin.clone());
    env.storage().instance().set(&DataKey::Config, &cfg);
    log_admin_action(&env, &admin_signers.get(0).unwrap(), "add_admin");
    env.events().publish((symbol_short!("admin"), symbol_short!("added")), new_admin);
}

pub fn remove_admin(env: Env, admin_signers: Vec<Address>, admin_to_remove: Address) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    let idx = cfg.admins.iter().position(|a| a == admin_to_remove).expect("not an admin") as u32;
    cfg.admins.remove(idx);
    assert!(!cfg.admins.is_empty(), "cannot remove last admin");
    assert!(cfg.admin_threshold <= cfg.admins.len(), "threshold invalid after removal");
    env.storage().instance().set(&DataKey::Config, &cfg);
    env.events().publish((symbol_short!("admin"), symbol_short!("removed")), admin_to_remove);
}

pub fn rotate_admin(env: Env, admin_signers: Vec<Address>, old_admin: Address, new_admin: Address) {
    require_admin_approval(&env, &admin_signers);
    assert!(old_admin != new_admin, "must differ");
    let mut cfg = config(&env);
    assert!(!cfg.admins.iter().any(|a| a == new_admin), "new admin already exists");
    let idx = cfg.admins.iter().position(|a| a == old_admin).expect("old admin not found") as u32;
    cfg.admins.set(idx, new_admin.clone());
    env.storage().instance().set(&DataKey::Config, &cfg);
    env.storage().persistent().remove(&DataKey::AdminKeyExpiry(new_admin.clone()));
    log_admin_action(&env, &admin_signers.get(0).unwrap(), "rotate_admin");
    env.events().publish((symbol_short!("admin"), symbol_short!("rotated")), (old_admin, new_admin));
}

pub fn set_admin_threshold(env: Env, admin_signers: Vec<Address>, new_threshold: u32) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    assert!(new_threshold > 0 && new_threshold <= cfg.admins.len(), "invalid threshold");
    cfg.admin_threshold = new_threshold;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn get_admins(env: Env) -> Vec<Address> {
    config(&env).admins
}

pub fn get_admin_threshold(env: Env) -> u32 {
    config(&env).admin_threshold
}

pub fn propose_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    if crate::helpers::is_zero_address(&env, &new_admin) {
        return Err(ContractError::ZeroAddress);
    }
    add_admin(env, admin_signers, new_admin);
    Ok(())
}

pub fn accept_admin(env: Env) -> Result<(), ContractError> {
    // No-op: propose_admin adds directly
    Ok(())
}

// ── Protocol fee ──────────────────────────────────────────────────────────────

pub fn set_protocol_fee(env: Env, admin_signers: Vec<Address>, fee_bps: u32) {
    require_admin_approval(&env, &admin_signers);
    assert!(fee_bps <= 10_000, "fee too high");
    env.storage().instance().set(&DataKey::ProtocolFeeBps, &fee_bps);
}

pub fn get_protocol_fee(env: Env) -> u32 {
    env.storage().instance().get(&DataKey::ProtocolFeeBps).unwrap_or(0)
}

// ── Fee treasury ──────────────────────────────────────────────────────────────

pub fn set_fee_treasury(env: Env, admin_signers: Vec<Address>, treasury: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::FeeTreasury, &treasury);
}

pub fn get_fee_treasury(env: Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::FeeTreasury)
}

// ── Voucher whitelist ─────────────────────────────────────────────────────────

pub fn whitelist_voucher(env: Env, admin_signers: Vec<Address>, voucher: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().set(&DataKey::VoucherWhitelist(voucher.clone()), &true);
    extend_ttl(&env, &DataKey::VoucherWhitelist(voucher));
}

pub fn add_voucher_to_whitelist(env: Env, admin_signers: Vec<Address>, voucher: Address) {
    whitelist_voucher(env, admin_signers, voucher)
}

pub fn remove_voucher_from_whitelist(env: Env, admin_signers: Vec<Address>, voucher: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().remove(&DataKey::VoucherWhitelist(voucher));
}

pub fn enable_voucher_whitelist(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::VoucherWhitelistEnabled, &true);
}

pub fn disable_voucher_whitelist(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::VoucherWhitelistEnabled, &false);
}

pub fn is_whitelisted(env: Env, voucher: Address) -> bool {
    env.storage().persistent().get(&DataKey::VoucherWhitelist(voucher)).unwrap_or(false)
}

pub fn is_voucher_whitelist_enabled(env: Env) -> bool {
    env.storage().instance().get(&DataKey::VoucherWhitelistEnabled).unwrap_or(false)
}

// ── Borrower whitelist ────────────────────────────────────────────────────────

pub fn add_borrower_to_whitelist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().set(&DataKey::BorrowerWhitelist(borrower.clone()), &true);
    extend_ttl(&env, &DataKey::BorrowerWhitelist(borrower));
}

pub fn remove_borrower_from_whitelist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().remove(&DataKey::BorrowerWhitelist(borrower));
}

pub fn enable_borrower_whitelist(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::BorrowerWhitelistEnabled, &true);
}

pub fn disable_borrower_whitelist(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::BorrowerWhitelistEnabled, &false);
}

pub fn is_borrower_whitelisted(env: Env, borrower: Address) -> bool {
    env.storage().persistent().get(&DataKey::BorrowerWhitelist(borrower)).unwrap_or(false)
}

pub fn is_borrower_whitelist_enabled(env: Env) -> bool {
    env.storage().instance().get(&DataKey::BorrowerWhitelistEnabled).unwrap_or(false)
}

// ── Config ────────────────────────────────────────────────────────────────────

pub fn set_config(env: Env, admin_signers: Vec<Address>, cfg: Config) {
    require_admin_approval(&env, &admin_signers);
    validate_admin_config(&env, &cfg.admins, cfg.admin_threshold).expect("invalid admin config");
    assert!(cfg.yield_bps >= 0 && cfg.yield_bps <= 10_000, "invalid yield bps");
    assert!(cfg.slash_bps >= 0 && cfg.slash_bps <= 10_000, "invalid slash bps");
    assert!(cfg.min_loan_amount > 0, "invalid min loan");
    assert!(cfg.loan_duration > 0, "invalid duration");
    assert!(cfg.grace_period <= cfg.loan_duration, "grace period exceeds loan duration");
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn update_config(env: Env, admin_signers: Vec<Address>, yield_bps: Option<i128>, slash_bps: Option<i128>) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    if let Some(y) = yield_bps {
        assert!((0..=10_000).contains(&y), "invalid yield");
        cfg.yield_bps = y;
    }
    if let Some(s) = slash_bps {
        assert!((0..=10_000).contains(&s), "invalid slash");
        cfg.slash_bps = s;
    }
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn get_config(env: Env) -> Config {
    config(&env)
}

pub fn set_reputation_nft(env: Env, admin_signers: Vec<Address>, nft_contract: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::ReputationNft, &nft_contract);
}

pub fn set_min_stake(env: Env, admin_signers: Vec<Address>, amount: i128) {
    require_admin_approval(&env, &admin_signers);
    assert!(amount >= 0, "invalid amount");
    env.storage().instance().set(&DataKey::MinStake, &amount);
}

pub fn get_min_stake(env: Env) -> i128 {
    env.storage().instance().get(&DataKey::MinStake).unwrap_or(0)
}

pub fn set_min_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    if amount <= 0 {
        return Err(ContractError::InvalidAmount);
    }
    let mut cfg = config(&env);
    cfg.min_loan_amount = amount;
    env.storage().instance().set(&DataKey::Config, &cfg);
    Ok(())
}

pub fn set_max_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) {
    require_admin_approval(&env, &admin_signers);
    assert!(amount >= 0, "invalid amount");
    env.storage().instance().set(&DataKey::MaxLoanAmount, &amount);
}

pub fn get_max_loan_amount(env: Env) -> i128 {
    env.storage().instance().get(&DataKey::MaxLoanAmount).unwrap_or(0)
}

pub fn set_min_vouchers(env: Env, admin_signers: Vec<Address>, count: u32) {
    require_admin_approval(&env, &admin_signers);
    // Store in config
    let mut cfg = config(&env);
    let _ = count; // stored separately if needed; no-op for now since MinVouchers removed
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn get_min_vouchers(env: Env) -> u32 {
    0 // MinVouchers variant removed; return 0 (no minimum)
}

pub fn set_max_loan_to_stake_ratio(env: Env, admin_signers: Vec<Address>, ratio: u32) {
    require_admin_approval(&env, &admin_signers);
    assert!(ratio > 0, "ratio must be > 0");
    let mut cfg = config(&env);
    cfg.max_loan_to_stake_ratio = ratio;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn get_max_loan_to_stake_ratio(env: Env) -> u32 {
    config(&env).max_loan_to_stake_ratio
}

pub fn set_grace_period(env: Env, admin_signers: Vec<Address>, period: u64) {
    require_admin_approval(&env, &admin_signers);
    assert!(period <= config(&env).loan_duration, "grace period exceeds loan duration");
    let mut cfg = config(&env);
    cfg.grace_period = period;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn add_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    require_valid_token(&env, &token)?;
    let mut cfg = config(&env);
    if cfg.token == token || cfg.allowed_tokens.iter().any(|t| t == token) {
        return Err(ContractError::DuplicateToken);
    }
    cfg.allowed_tokens.push_back(token);
    env.storage().instance().set(&DataKey::Config, &cfg);
    Ok(())
}

pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    if let Some(idx) = cfg.allowed_tokens.iter().position(|t| t == token) {
        cfg.allowed_tokens.remove(idx as u32);
        env.storage().instance().set(&DataKey::Config, &cfg);
    }
}

pub fn set_token_config(env: Env, admin_signers: Vec<Address>, token: Address, token_cfg: TokenConfig) {
    require_admin_approval(&env, &admin_signers);
    assert!(token_cfg.yield_bps <= 10_000, "invalid yield");
    assert!(token_cfg.slash_bps <= 10_000, "invalid slash");
    env.storage().persistent().set(&DataKey::TokenConfig(token.clone()), &token_cfg);
    extend_ttl(&env, &DataKey::TokenConfig(token));
}

pub fn get_token_config(env: Env, token: Address) -> Option<TokenConfig> {
    env.storage().persistent().get(&DataKey::TokenConfig(token))
}

// ── Blacklist ─────────────────────────────────────────────────────────────────

pub fn blacklist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().set(&DataKey::Blacklisted(borrower.clone()), &true);
    extend_ttl(&env, &DataKey::Blacklisted(borrower));
}

pub fn is_blacklisted(env: Env, borrower: Address) -> bool {
    env.storage().persistent().get(&DataKey::Blacklisted(borrower)).unwrap_or(false)
}

// ── Upgrade ───────────────────────────────────────────────────────────────────

pub fn upgrade(env: Env, admin_signers: Vec<Address>, new_wasm_hash: BytesN<32>) {
    require_admin_approval(&env, &admin_signers);
    
    // Perform pre-upgrade safety checks
    crate::upgrade::validate_upgrade(&env, new_wasm_hash.clone())
        .expect("Upgrade validation failed");
    
    // Perform pre-upgrade health check
    crate::upgrade::pre_upgrade_health_check(&env)
        .expect("Pre-upgrade health check failed");
    
    // Execute upgrade
    env.deployer().update_current_contract_wasm(new_wasm_hash.clone());
    
    // Perform post-upgrade verification
    crate::upgrade::post_upgrade_verification(&env)
        .expect("Post-upgrade verification failed");
    
    env.events().publish((symbol_short!("upgrade"),), new_wasm_hash);
    log_admin_action(&env, &admin_signers.get(0).unwrap(), "upgrade");
}

// ── Pause ─────────────────────────────────────────────────────────────────────

pub fn pause(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::Paused, &true);
    log_admin_action(&env, &admin_signers.get(0).unwrap(), "pause");
}

pub fn unpause(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::Paused, &false);
}

/// Pause a specific function by name (granular pause).
pub fn pause_function(env: Env, admin_signers: Vec<Address>, function_name: String) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    let flag = PauseFlag::from_string(&env, &function_name).ok_or(ContractError::InvalidAmount)?;
    crate::helpers::set_paused_for(&env, flag, true);
    Ok(())
}

/// Unpause a specific function by name.
pub fn unpause_function(env: Env, admin_signers: Vec<Address>, function_name: String) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    let flag = PauseFlag::from_string(&env, &function_name).ok_or(ContractError::InvalidAmount)?;
    crate::helpers::set_paused_for(&env, flag, false);
    Ok(())
}

/// Check if a specific function is paused.
pub fn get_pause_status(env: Env, function_name: String) -> bool {
    let flag = match PauseFlag::from_string(&env, &function_name) {
        Some(f) => f,
        None => return false,
    };
    crate::helpers::is_paused_for(&env, flag)
}

// ── Admin audit log ───────────────────────────────────────────────────────────

pub fn get_admin_audit_log(env: Env) -> Vec<AdminAuditEntry> {
    env.storage().instance().get(&DataKey::AdminAuditLog).unwrap_or(Vec::new(&env))
}

// ── Admin key expiry ──────────────────────────────────────────────────────────

pub fn set_admin_key_expiry(env: Env, admin_signers: Vec<Address>, admin: Address, expiry: u64) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().set(&DataKey::AdminKeyExpiry(admin.clone()), &expiry);
    extend_ttl(&env, &DataKey::AdminKeyExpiry(admin));
}

pub fn get_admin_key_expiry(env: Env, admin: Address) -> u64 {
    env.storage().persistent().get(&DataKey::AdminKeyExpiry(admin)).unwrap_or(0)
}

// ── Admin timelock ────────────────────────────────────────────────────────────

pub fn queue_admin_action(
    env: Env,
    admin_signers: Vec<Address>,
    action: AdminTimelockAction,
    delay_secs: u64,
) -> Result<u64, ContractError> {
    require_admin_approval(&env, &admin_signers);
    let id: u64 = env.storage().instance().get(&DataKey::TimelockCounter).unwrap_or(0) + 1;
    env.storage().instance().set(&DataKey::TimelockCounter, &id);
    let eta = env.ledger().timestamp() + delay_secs;
    let timelock = AdminTimelock {
        id,
        action,
        proposer: admin_signers.get(0).unwrap(),
        eta,
        executed: false,
        cancelled: false,
    };
    env.storage().persistent().set(&DataKey::Timelock(id), &timelock);
    extend_ttl(&env, &DataKey::Timelock(id));
    Ok(id)
}

pub fn execute_admin_action(env: Env, action_id: u64) -> Result<(), ContractError> {
    let mut timelock: AdminTimelock = env
        .storage()
        .persistent()
        .get(&DataKey::Timelock(action_id))
        .ok_or(ContractError::TimelockNotFound)?;
    if timelock.executed || timelock.cancelled {
        return Err(ContractError::InvalidStateTransition);
    }
    if env.ledger().timestamp() < timelock.eta {
        return Err(ContractError::TimelockNotReady);
    }
    timelock.executed = true;
    env.storage().persistent().set(&DataKey::Timelock(action_id), &timelock);
    // Execute the action
    match timelock.action {
        AdminTimelockAction::Pause => {
            env.storage().instance().set(&DataKey::Paused, &true);
        }
        AdminTimelockAction::Unpause => {
            env.storage().instance().set(&DataKey::Paused, &false);
        }
        AdminTimelockAction::UpdateConfig(cfg) => {
            env.storage().instance().set(&DataKey::Config, &cfg);
        }
        AdminTimelockAction::SetAdminThreshold(t) => {
            let mut cfg = config(&env);
            cfg.admin_threshold = t;
            env.storage().instance().set(&DataKey::Config, &cfg);
        }
    }
    Ok(())
}

pub fn cancel_admin_action(env: Env, caller: Address, action_id: u64) -> Result<(), ContractError> {
    caller.require_auth();
    let mut timelock: AdminTimelock = env
        .storage()
        .persistent()
        .get(&DataKey::Timelock(action_id))
        .ok_or(ContractError::TimelockNotFound)?;
    if timelock.executed || timelock.cancelled {
        return Err(ContractError::InvalidStateTransition);
    }
    timelock.cancelled = true;
    env.storage().persistent().set(&DataKey::Timelock(action_id), &timelock);
    Ok(())
}

pub fn get_admin_timelock(env: Env, action_id: u64) -> Option<AdminTimelock> {
    env.storage().persistent().get(&DataKey::Timelock(action_id))
}

// ── Voucher stake limit ───────────────────────────────────────────────────────

pub fn set_voucher_stake_limit(
    env: Env,
    admin_signers: Vec<Address>,
    voucher: Address,
    borrower: Address,
    limit: i128,
) {
    require_admin_approval(&env, &admin_signers);
    let key = DataKey::VoucherStakeLimit(VoucherStakeLimitKey { voucher: voucher.clone(), borrower: borrower.clone() });
    env.storage().persistent().set(&key, &limit);
    extend_ttl(&env, &key);
}

pub fn get_voucher_stake_limit(env: Env, voucher: Address, borrower: Address) -> Option<i128> {
    env.storage().persistent().get(&DataKey::VoucherStakeLimit(VoucherStakeLimitKey { voucher, borrower }))
}

// ── Governance token ──────────────────────────────────────────────────────────

pub fn set_governance_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    require_valid_token(&env, &token)?;
    env.storage().instance().set(&DataKey::GovernanceToken, &token);
    Ok(())
}

pub fn get_admins(env: Env) -> Vec<Address> {
    config(&env).admins
}

pub fn get_admin_threshold(env: Env) -> u32 {
    config(&env).admin_threshold
}

pub fn is_whitelisted(env: Env, voucher: Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::VoucherWhitelist(voucher))
        .unwrap_or(false)
}

pub fn is_voucher_whitelist_enabled(env: Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::VoucherWhitelistEnabled)
        .unwrap_or(false)
}

pub fn is_borrower_whitelisted(env: Env, borrower: Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::BorrowerWhitelist(borrower))
        .unwrap_or(false)
}

pub fn is_borrower_whitelist_enabled(env: Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::BorrowerWhitelistEnabled)
        .unwrap_or(false)
}

pub fn get_fee_treasury(env: Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::FeeTreasury)
}

pub fn get_min_stake(env: Env) -> i128 {
    env.storage().instance().get(&DataKey::MinStake).unwrap_or(0)
}

pub fn get_max_loan_amount(env: Env) -> i128 {
    env.storage().instance().get(&DataKey::MaxLoanAmount).unwrap_or(0)
}

pub fn get_min_vouchers(env: Env) -> u32 {
    env.storage().instance().get(&DataKey::MinVouchers).unwrap_or(0)
}

pub fn get_max_loan_to_stake_ratio(env: Env) -> u32 {
    config(&env).max_loan_to_stake_ratio
}

pub fn set_min_stake(env: Env, admin_signers: Vec<Address>, amount: i128) {
    require_admin_approval(&env, &admin_signers);
    assert!(amount >= 0, "min stake must be non-negative");
    env.storage().instance().set(&DataKey::MinStake, &amount);
}

pub fn set_min_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) -> Result<(), crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    if amount <= 0 {
        return Err(crate::errors::ContractError::InvalidAmount);
    }
    let mut cfg = config(&env);
    cfg.min_loan_amount = amount;
    env.storage().instance().set(&DataKey::Config, &cfg);
    Ok(())
}

pub fn set_max_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::MaxLoanAmount, &amount);
}

pub fn set_min_vouchers(env: Env, admin_signers: Vec<Address>, count: u32) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::MinVouchers, &count);
}

pub fn set_max_loan_to_stake_ratio(env: Env, admin_signers: Vec<Address>, ratio: u32) {
    require_admin_approval(&env, &admin_signers);
    assert!(ratio > 0, "ratio must be positive");
    let mut cfg = config(&env);
    cfg.max_loan_to_stake_ratio = ratio;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn set_grace_period(env: Env, admin_signers: Vec<Address>, period: u64) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    assert!(period <= cfg.loan_duration, "grace period cannot exceed loan duration");
    cfg.grace_period = period;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn add_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    require_valid_token(&env, &token)?;
    let mut cfg = config(&env);
    if cfg.token == token || cfg.allowed_tokens.iter().any(|t| t == token) {
        return Err(crate::errors::ContractError::DuplicateToken);
    }
    cfg.allowed_tokens.push_back(token);
    env.storage().instance().set(&DataKey::Config, &cfg);
    Ok(())
}

pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    if let Some(idx) = cfg.allowed_tokens.iter().position(|t| t == token) {
        cfg.allowed_tokens.remove(idx as u32);
        env.storage().instance().set(&DataKey::Config, &cfg);
    }
}

pub fn set_reputation_nft(env: Env, admin_signers: Vec<Address>, nft_contract: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::ReputationNft, &nft_contract);
}

pub fn propose_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) -> Result<(), crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    if is_zero_address(&env, &new_admin) {
        return Err(crate::errors::ContractError::ZeroAddress);
    }
    env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
    Ok(())
}

pub fn accept_admin(env: Env) -> Result<(), crate::errors::ContractError> {
    let pending: Address = env
        .storage()
        .instance()
        .get(&DataKey::PendingAdmin)
        .ok_or(crate::errors::ContractError::UnauthorizedCaller)?;
    pending.require_auth();
    let mut cfg = config(&env);
    cfg.admins.push_back(pending.clone());
    env.storage().instance().set(&DataKey::Config, &cfg);
    env.storage().instance().remove(&DataKey::PendingAdmin);
    Ok(())
}

pub fn pause_function(env: Env, admin_signers: Vec<Address>, function_name: soroban_sdk::String) -> Result<(), crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    let flag = crate::types::PauseFlag::from_string(&env, &function_name)
        .ok_or(crate::errors::ContractError::InvalidAmount)?;
    env.storage().instance().set(&DataKey::PauseFlag(flag), &true);
    Ok(())
}

pub fn unpause_function(env: Env, admin_signers: Vec<Address>, function_name: soroban_sdk::String) -> Result<(), crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    let flag = crate::types::PauseFlag::from_string(&env, &function_name)
        .ok_or(crate::errors::ContractError::InvalidAmount)?;
    env.storage().instance().set(&DataKey::PauseFlag(flag), &false);
    Ok(())
}

pub fn get_pause_status(env: Env, function_name: soroban_sdk::String) -> bool {
    let flag = match crate::types::PauseFlag::from_string(&env, &function_name) {
        Some(f) => f,
        None => return false,
    };
    env.storage().instance().get(&DataKey::PauseFlag(flag)).unwrap_or(false)
}

pub fn is_admin_key_expired(env: &Env, admin: &Address) -> bool {
    let expiry: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::AdminKeyExpiry(admin.clone()))
        .unwrap_or(0);
    expiry > 0 && env.ledger().timestamp() > expiry
}

pub fn set_admin_key_expiry(env: Env, admin_signers: Vec<Address>, admin: Address, expiry: u64) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().set(&DataKey::AdminKeyExpiry(admin.clone()), &expiry);
    extend_ttl(&env, &DataKey::AdminKeyExpiry(admin));
}

pub fn get_admin_key_expiry(env: Env, admin: Address) -> u64 {
    env.storage().persistent().get(&DataKey::AdminKeyExpiry(admin)).unwrap_or(0)
}

pub fn get_admin_audit_log(env: Env) -> Vec<crate::types::AdminAuditEntry> {
    env.storage().instance().get(&DataKey::AdminAuditLog).unwrap_or(Vec::new(&env))
}

pub fn queue_admin_action(
    env: Env,
    admin_signers: Vec<Address>,
    action: crate::types::AdminTimelockAction,
    delay_secs: u64,
) -> Result<u64, crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    let id: u64 = env.storage().instance().get(&DataKey::AdminActionTimelockCounter).unwrap_or(0) + 1;
    env.storage().instance().set(&DataKey::AdminActionTimelockCounter, &id);
    let eta = env.ledger().timestamp() + delay_secs;
    let timelock = crate::types::AdminTimelock {
        id,
        action,
        proposer: admin_signers.get(0).unwrap(),
        eta,
        executed: false,
        cancelled: false,
    };
    env.storage().persistent().set(&DataKey::AdminActionTimelock(id), &timelock);
    extend_ttl(&env, &DataKey::AdminActionTimelock(id));
    Ok(id)
}

pub fn execute_admin_action(env: Env, action_id: u64) -> Result<(), crate::errors::ContractError> {
    let mut timelock: crate::types::AdminTimelock = env
        .storage()
        .persistent()
        .get(&DataKey::AdminActionTimelock(action_id))
        .ok_or(crate::errors::ContractError::TimelockNotFound)?;
    if timelock.executed || timelock.cancelled {
        return Err(crate::errors::ContractError::InvalidStateTransition);
    }
    if env.ledger().timestamp() < timelock.eta {
        return Err(crate::errors::ContractError::TimelockNotReady);
    }
    timelock.executed = true;
    env.storage().persistent().set(&DataKey::AdminActionTimelock(action_id), &timelock);
    Ok(())
}

pub fn cancel_admin_action(env: Env, caller: Address, action_id: u64) -> Result<(), crate::errors::ContractError> {
    caller.require_auth();
    let mut timelock: crate::types::AdminTimelock = env
        .storage()
        .persistent()
        .get(&DataKey::AdminActionTimelock(action_id))
        .ok_or(crate::errors::ContractError::TimelockNotFound)?;
    timelock.cancelled = true;
    env.storage().persistent().set(&DataKey::AdminActionTimelock(action_id), &timelock);
    Ok(())
}

pub fn get_admin_timelock(env: Env, action_id: u64) -> Option<crate::types::AdminTimelock> {
    env.storage().persistent().get(&DataKey::AdminActionTimelock(action_id))
}

pub fn set_governance_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), crate::errors::ContractError> {
    require_admin_approval(&env, &admin_signers);
    require_valid_token(&env, &token)?;
    env.storage().instance().set(&DataKey::GovernanceTokenAddress, &token);
    Ok(())
}

pub fn set_voucher_stake_limit(env: Env, admin_signers: Vec<Address>, voucher: Address, borrower: Address, limit: i128) {
    require_admin_approval(&env, &admin_signers);
    env.storage().persistent().set(&DataKey::VoucherStakeLimit(voucher.clone(), borrower.clone()), &limit);
    extend_ttl(&env, &DataKey::VoucherStakeLimit(voucher, borrower));
}

pub fn get_voucher_stake_limit(env: Env, voucher: Address, borrower: Address) -> Option<i128> {
    env.storage().persistent().get(&DataKey::VoucherStakeLimit(voucher, borrower))
}

pub fn add_voucher_to_whitelist(env: Env, admin_signers: Vec<Address>, voucher: Address) {
    whitelist_voucher(env, admin_signers, voucher)
}

fn log_admin_action(env: &Env, admin: &Address, action: &str) {
    let mut log: Vec<crate::types::AdminAuditEntry> = env
        .storage()
        .instance()
        .get(&DataKey::AdminAuditLog)
        .unwrap_or(Vec::new(env));
    log.push_back(crate::types::AdminAuditEntry {
        admin: admin.clone(),
        action: soroban_sdk::String::from_str(env, action),
        timestamp: env.ledger().timestamp(),
    });
    env.storage().instance().set(&DataKey::AdminAuditLog, &log);
}

// #643: Set allowed loan purposes whitelist
pub fn set_allowed_purposes(env: Env, admin_signers: Vec<Address>, purposes: Vec<soroban_sdk::String>) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    cfg.allowed_purposes = purposes;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

// #644: Set insurance premium in basis points
pub fn set_insurance_premium_bps(env: Env, admin_signers: Vec<Address>, bps: i128) {
    require_admin_approval(&env, &admin_signers);
    assert!(bps >= 0 && bps <= 10_000, "insurance_premium_bps must be 0-10000");
    let mut cfg = config(&env);
    cfg.insurance_premium_bps = bps;
    env.storage().instance().set(&DataKey::Config, &cfg);
}
