use crate::helpers::{config, require_admin_approval, require_valid_token, validate_admin_config};
use crate::types::{Config, DataKey, AdminActionProposal};
use soroban_sdk::{panic_with_error, symbol_short, Address, BytesN, Env, Vec};
use crate::errors::ContractError;

pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
    require_admin_approval(&env, &admin_signers);

    let mut cfg = config(&env);

    if cfg.admins.iter().any(|a| a == new_admin) {
        panic_with_error!(&env, ContractError::AlreadyInitialized);
    }

    cfg.admins.push_back(new_admin.clone());
    env.storage().instance().set(&DataKey::Config, &cfg);

    env.events()
        .publish((symbol_short!("admin"), symbol_short!("added")), new_admin);
}

pub fn remove_admin(env: Env, admin_signers: Vec<Address>, admin_to_remove: Address) {
    require_admin_approval(&env, &admin_signers);

    // Issue #372: Prevent removing an admin who is one of the signers
    for signer in admin_signers.iter() {
        if signer == admin_to_remove {
            panic_with_error!(&env, ContractError::UnauthorizedCaller);
        }
    }

    let mut cfg = config(&env);

    let idx = cfg
        .admins
        .iter()
        .position(|a| a == admin_to_remove)
        .expect("address is not an admin") as u32;

    cfg.admins.remove(idx);

    if cfg.admins.is_empty() {
        panic_with_error!(&env, ContractError::UnauthorizedCaller);
    }
    if cfg.admin_threshold > cfg.admins.len() {
        panic_with_error!(&env, ContractError::InvalidAdminThreshold);
    }

    env.storage().instance().set(&DataKey::Config, &cfg);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("removed")),
        admin_to_remove,
    );
}

pub fn rotate_admin(env: Env, admin_signers: Vec<Address>, old_admin: Address, new_admin: Address) {
    require_admin_approval(&env, &admin_signers);

    if old_admin == new_admin {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }

    let mut cfg = config(&env);

    if cfg.admins.iter().any(|a| a == new_admin) {
        panic_with_error!(&env, ContractError::AlreadyInitialized);
    }

    let idx = cfg
        .admins
        .iter()
        .position(|a| a == old_admin)
        .expect("old admin not found") as u32;

    cfg.admins.set(idx, new_admin.clone());
    env.storage().instance().set(&DataKey::Config, &cfg);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("rotated")),
        (old_admin, new_admin),
    );
}

pub fn set_admin_threshold(env: Env, admin_signers: Vec<Address>, new_threshold: u32) {
    require_admin_approval(&env, &admin_signers);

    let mut cfg = config(&env);

    if new_threshold == 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    if new_threshold > cfg.admins.len() {
        panic_with_error!(&env, ContractError::InvalidAdminThreshold);
    }

    cfg.admin_threshold = new_threshold;
    env.storage().instance().set(&DataKey::Config, &cfg);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("thresh")),
        new_threshold,
    );
}

pub fn set_protocol_fee(env: Env, admin_signers: Vec<Address>, fee_bps: u32) {
    require_admin_approval(&env, &admin_signers);
    if fee_bps > 10_000 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    env.storage()
        .instance()
        .set(&DataKey::ProtocolFeeBps, &fee_bps);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("fee")),
        (
            admin_signers.get(0).unwrap(),
            fee_bps,
            env.ledger().timestamp(),
        ),
    );
}

pub fn whitelist_voucher(env: Env, admin_signers: Vec<Address>, voucher: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage()
        .persistent()
        .set(&DataKey::VoucherWhitelist(voucher), &true);
}

pub fn set_whitelist_enabled(env: Env, admin_signers: Vec<Address>, enabled: bool) {
    require_admin_approval(&env, &admin_signers);
    env.storage()
        .instance()
        .set(&DataKey::WhitelistEnabled, &enabled);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("wlena")),
        (admin_signers.get(0).unwrap(), enabled),
    );
}

pub fn set_fee_treasury(env: Env, admin_signers: Vec<Address>, treasury: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage()
        .instance()
        .set(&DataKey::FeeTreasury, &treasury);
}

pub fn upgrade(env: Env, admin_signers: Vec<Address>, new_wasm_hash: BytesN<32>) {
    require_admin_approval(&env, &admin_signers);
    env.deployer()
        .update_current_contract_wasm(new_wasm_hash.clone());
    env.events()
        .publish((symbol_short!("upgrade"),), new_wasm_hash);
}

pub fn pause(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::Paused, &true);
    env.storage().instance().set(&DataKey::PauseMode, &crate::types::PauseMode::Paused);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("pause")),
        (admin_signers.get(0).unwrap(), env.ledger().timestamp()),
    );
}

pub fn unpause(env: Env, admin_signers: Vec<Address>) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::Paused, &false);
    env.storage().instance().set(&DataKey::PauseMode, &crate::types::PauseMode::None);
    env.storage().instance().remove(&DataKey::ThawState);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("unpause")),
        (admin_signers.get(0).unwrap(), env.ledger().timestamp()),
    );
}

/// Pause the contract with a gradual thaw period allowing emergency withdrawals
pub fn pause_with_thaw(env: Env, admin_signers: Vec<Address>, thaw_duration: u64) {
    require_admin_approval(&env, &admin_signers);
    let now = env.ledger().timestamp();
    
    env.storage().instance().set(&DataKey::Paused, &true);
    env.storage().instance().set(&DataKey::PauseMode, &crate::types::PauseMode::Thawing);
    env.storage().instance().set(
        &DataKey::ThawState,
        &crate::types::ThawState {
            pause_timestamp: now,
            thaw_duration,
            thaw_start_timestamp: now,
        },
    );
    
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("pse_thaw")),
        (admin_signers.get(0).unwrap(), now, thaw_duration),
    );
}

/// Check if contract is in thaw period and allow emergency withdrawals
pub fn is_in_thaw_period(env: &Env) -> bool {
    if let Some(thaw_state) = env.storage().instance().get::<_, crate::types::ThawState>(&DataKey::ThawState) {
        let now = env.ledger().timestamp();
        let thaw_end = thaw_state.thaw_start_timestamp + thaw_state.thaw_duration;
        now <= thaw_end
    } else {
        false
    }
}

pub fn blacklist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage()
        .persistent()
        .set(&DataKey::Blacklisted(borrower), &true);
}

pub fn set_config(env: Env, admin_signers: Vec<Address>, config: Config) {
    require_admin_approval(&env, &admin_signers);
    validate_admin_config(&env, &config.admins, config.admin_threshold)
        .expect("invalid admin config");
    if config.yield_bps < 0 || config.yield_bps > 10_000 {
        panic_with_error!(&env, ContractError::InvalidBps);
    }
    if config.slash_bps <= 0 || config.slash_bps > 10_000 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    if config.max_vouchers == 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    if config.min_loan_amount <= 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    if config.loan_duration == 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    if config.grace_period > config.loan_duration {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    if config.max_loan_to_stake_ratio == 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    env.storage().instance().set(&DataKey::Config, &config);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("config")),
        (admin_signers.get(0).unwrap(), env.ledger().timestamp()),
    );
}

pub fn update_config(
    env: Env,
    admin_signers: Vec<Address>,
    yield_bps: Option<i128>,
    slash_bps: Option<i128>,
) {
    require_admin_approval(&env, &admin_signers);

    let mut cfg = config(&env);

    if let Some(new_yield_bps) = yield_bps {
        if new_yield_bps < 0 || new_yield_bps > 10_000 {
            panic_with_error!(&env, ContractError::InvalidBps);
        }
        cfg.yield_bps = new_yield_bps;
    }

    if let Some(new_slash_bps) = slash_bps {
        if new_slash_bps <= 0 || new_slash_bps > 10_000 {
            panic_with_error!(&env, ContractError::InvalidAmount);
        }
        cfg.slash_bps = new_slash_bps;
    }

    env.storage().instance().set(&DataKey::Config, &cfg);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("upconfig")),
        (admin_signers.get(0).unwrap(), env.ledger().timestamp()),
    );
}

/// Toggle dynamic slash threshold on/off.
/// When enabled, slash penalties adjust based on protocol health.
/// When disabled, uses static slash_bps from Config.
pub fn set_dynamic_slash_threshold(
    env: Env,
    admin_signers: Vec<Address>,
    enabled: bool,
) {
    require_admin_approval(&env, &admin_signers);

    let mut cfg = config(&env);
    cfg.dynamic_slash_threshold = enabled;

    env.storage().instance().set(&DataKey::Config, &cfg);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("dynslash")),
        (admin_signers.get(0).unwrap(), enabled, env.ledger().timestamp()),
    );
}

/// Get the current effective slash threshold (either static or dynamic).
/// This function can be called by anyone to see what slash rate would be applied.
pub fn get_effective_slash_threshold(env: Env) -> i128 {
    crate::helpers::calculate_dynamic_slash_threshold(&env)
}

pub fn set_reputation_nft(env: Env, admin_signers: Vec<Address>, nft_contract: Address) {
    require_admin_approval(&env, &admin_signers);
    env.storage()
        .instance()
        .set(&DataKey::ReputationNft, &nft_contract);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("repnft")),
        (
            admin_signers.get(0).unwrap(),
            nft_contract,
            env.ledger().timestamp(),
        ),
    );
}

/// Set the minimum allowed vouch stake.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `admin_signers` - Admin addresses authorizing this call (must meet threshold)
/// * `amount` - Minimum stake amount, in stroops (0 disables the minimum check).
///   1 XLM = 10,000,000 stroops.
pub fn set_min_stake(env: Env, admin_signers: Vec<Address>, amount: i128) {
    require_admin_approval(&env, &admin_signers);
    if amount < 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    env.storage().instance().set(&DataKey::MinStake, &amount);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("minstake")),
        (
            admin_signers.get(0).unwrap(),
            amount,
            env.ledger().timestamp(),
        ),
    );
}

/// Set the maximum loan amount allowed per loan request.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `admin_signers` - Admin addresses authorizing this call (must meet threshold)
/// * `amount` - Maximum loan amount, in stroops (0 = no cap enforced).
///   1 XLM = 10,000,000 stroops.
pub fn set_max_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) {
    require_admin_approval(&env, &admin_signers);
    if amount < 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    env.storage()
        .instance()
        .set(&DataKey::MaxLoanAmount, &amount);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("maxloan")),
        (
            admin_signers.get(0).unwrap(),
            amount,
            env.ledger().timestamp(),
        ),
    );
}

pub fn set_min_vouchers(env: Env, admin_signers: Vec<Address>, count: u32) {
    require_admin_approval(&env, &admin_signers);
    env.storage().instance().set(&DataKey::MinVouchers, &count);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("minvchrs")),
        (
            admin_signers.get(0).unwrap(),
            count,
            env.ledger().timestamp(),
        ),
    );
}

pub fn set_max_loan_to_stake_ratio(env: Env, admin_signers: Vec<Address>, ratio: u32) {
    require_admin_approval(&env, &admin_signers);
    if ratio == 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    let mut cfg = config(&env);
    cfg.max_loan_to_stake_ratio = ratio;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn set_grace_period(env: Env, admin_signers: Vec<Address>, period: u64) {
    require_admin_approval(&env, &admin_signers);
    let cfg = config(&env);
    if period > cfg.loan_duration {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    let mut cfg = cfg;
    cfg.grace_period = period;
    env.storage().instance().set(&DataKey::Config, &cfg);
}

// View functions
pub fn get_protocol_fee(env: Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ProtocolFeeBps)
        .unwrap_or(0)
}

pub fn get_fee_treasury(env: Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::FeeTreasury)
}

pub fn is_blacklisted(env: Env, borrower: Address) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(borrower))
        .unwrap_or(false)
}

pub fn get_min_stake(env: Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MinStake)
        .unwrap_or(0)
}

pub fn get_max_loan_amount(env: Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MaxLoanAmount)
        .unwrap_or(0)
}

pub fn get_min_vouchers(env: Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MinVouchers)
        .unwrap_or(0)
}

pub fn get_max_loan_to_stake_ratio(env: Env) -> u32 {
    config(&env).max_loan_to_stake_ratio
}

pub fn get_config(env: Env) -> Config {
    config(&env)
}

pub fn add_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);
    require_valid_token(&env, &token)?;
    let mut cfg = config(&env);
    if cfg.allowed_tokens.iter().any(|t| t == token) || token == cfg.token {
        return Err(ContractError::DuplicateToken);
    }
    cfg.allowed_tokens.push_back(token);
    env.storage().instance().set(&DataKey::Config, &cfg);
    Ok(())
}

pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
    require_admin_approval(&env, &admin_signers);
    let mut cfg = config(&env);
    let idx = cfg
        .allowed_tokens
        .iter()
        .position(|t| t == token)
        .expect("token not in allowed list") as u32;
    cfg.allowed_tokens.remove(idx);
    env.storage().instance().set(&DataKey::Config, &cfg);
}

pub fn get_admins(env: Env) -> Vec<Address> {
    crate::helpers::get_admins(&env)
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

pub fn is_whitelist_enabled(env: Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::WhitelistEnabled)
        .unwrap_or(false)
}

pub fn set_max_vouchers_per_borrower(env: Env, admin_signers: Vec<Address>, max_vouchers: u32) {
    require_admin_approval(&env, &admin_signers);
    if max_vouchers == 0 {
        panic_with_error!(&env, ContractError::InvalidAmount);
    }
    env.storage()
        .instance()
        .set(&DataKey::MaxVouchersPerBorrower, &max_vouchers);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("maxvchbr")),
        (
            admin_signers.get(0).unwrap(),
            max_vouchers,
            env.ledger().timestamp(),
        ),
    );
}

pub fn get_max_vouchers_per_borrower(env: Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MaxVouchersPerBorrower)
        .unwrap_or(crate::types::DEFAULT_MAX_VOUCHERS_PER_BORROWER)
}

pub fn withdraw_slash_treasury(
    env: Env,
    admin_signers: Vec<Address>,
    recipient: Address,
    amount: i128,
) {
    require_admin_approval(&env, &admin_signers);
    assert!(amount > 0, "amount must be greater than zero");

    let balance: i128 = env
        .storage()
        .instance()
        .get(&DataKey::SlashTreasury)
        .unwrap_or(0);
    assert!(balance >= amount, "insufficient slash treasury balance");

    env.storage()
        .instance()
        .set(&DataKey::SlashTreasury, &(balance - amount));

    let cfg = config(&env);
    soroban_sdk::token::Client::new(&env, &cfg.token)
        .transfer(&env.current_contract_address(), &recipient, &amount);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("slshwdraw")),
        (admin_signers.get(0).unwrap(), recipient, amount),
    );
}

pub fn propose_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) -> Result<(), ContractError> {
    require_admin_approval(&env, &admin_signers);

    if new_admin == Address::from_str(&env, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF") {
        return Err(ContractError::ZeroAddress);
    }

    env.storage()
        .instance()
        .set(&DataKey::PendingAdmin, &new_admin);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("proposed")),
        new_admin,
    );

    Ok(())
}

pub fn accept_admin(env: Env) -> Result<(), ContractError> {
    let new_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::PendingAdmin)
        .ok_or(ContractError::UnauthorizedCaller)?;

    new_admin.require_auth();

    let mut cfg = config(&env);
    cfg.admins.push_back(new_admin.clone());
    env.storage().instance().set(&DataKey::Config, &cfg);

    // Clear the pending admin
    env.storage().instance().remove(&DataKey::PendingAdmin);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("accepted")),
        new_admin,
    );

    Ok(())
}

pub fn set_prepayment_penalty_bps(env: Env, admin_signers: Vec<Address>, penalty_bps: u32) {
    require_admin_approval(&env, &admin_signers);
    assert!(penalty_bps <= 10_000, "penalty_bps must not exceed 10000");
    env.storage()
        .instance()
        .set(&DataKey::PrepaymentPenaltyBps, &penalty_bps);
    env.events().publish(
        (symbol_short!("admin"), symbol_short!("prepay")),
        (admin_signers.get(0).unwrap(), penalty_bps),
    );
}

pub fn get_prepayment_penalty_bps(env: Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::PrepaymentPenaltyBps)
        .unwrap_or(0)
}

/// Issue #554: Propose an admin action (e.g., pause, slash, config change).
pub fn propose_admin_action(
    env: Env,
    proposer: Address,
    action_type: soroban_sdk::String,
) -> Result<u64, ContractError> {
    proposer.require_auth();

    let action_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::AdminActionCounter)
        .unwrap_or(0u64)
        .checked_add(1)
        .expect("action ID overflow");

    let proposal = AdminActionProposal {
        id: action_id,
        action_type,
        proposer: proposer.clone(),
        approvals: Vec::new(&env),
        created_at: env.ledger().timestamp(),
        executed: false,
    };

    env.storage()
        .instance()
        .set(&DataKey::AdminAction(action_id), &proposal);
    env.storage()
        .instance()
        .set(&DataKey::AdminActionCounter, &action_id);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("propose")),
        (action_id, proposer),
    );

    Ok(action_id)
}

/// Issue #554: Approve an admin action. Requires admin signature.
pub fn approve_admin_action(
    env: Env,
    admin: Address,
    action_id: u64,
) -> Result<(), ContractError> {
    admin.require_auth();

    let cfg = config(&env);
    if !cfg.admins.iter().any(|a| a == admin) {
        return Err(ContractError::UnauthorizedCaller);
    }

    let mut proposal: AdminActionProposal = env
        .storage()
        .instance()
        .get(&DataKey::AdminAction(action_id))
        .ok_or(ContractError::NoActiveLoan)?;

    if proposal.executed {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    // Prevent double-approval
    if proposal.approvals.iter().any(|a| a == admin) {
        return Err(ContractError::AlreadyVoted);
    }

    proposal.approvals.push_back(admin.clone());

    env.storage()
        .instance()
        .set(&DataKey::AdminAction(action_id), &proposal);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("approve")),
        (action_id, admin),
    );

    Ok(())
}

/// Issue #554: Execute an admin action if threshold is met.
pub fn execute_admin_action(env: Env, action_id: u64) -> Result<(), ContractError> {
    let mut proposal: AdminActionProposal = env
        .storage()
        .instance()
        .get(&DataKey::AdminAction(action_id))
        .ok_or(ContractError::NoActiveLoan)?;

    if proposal.executed {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    let cfg = config(&env);
    if proposal.approvals.len() < cfg.admin_threshold {
        return Err(ContractError::UnauthorizedCaller);
    }

    proposal.executed = true;
    env.storage()
        .instance()
        .set(&DataKey::AdminAction(action_id), &proposal);

    env.events().publish(
        (symbol_short!("admin"), symbol_short!("execute")),
        (action_id, proposal.action_type.clone()),
    );

    Ok(())
}
