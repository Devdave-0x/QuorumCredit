use crate::errors::ContractError;
use crate::helpers::{
    extend_ttl, has_active_loan, require_allowed_token, require_not_paused, 
    require_not_paused_for, require_positive_amount,
};
use crate::types::{DataKey, PauseFlag, VouchRecord, MAX_VOUCH_DEPTH};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, Vec};

// Task 3: Circular Vouch Detection - Detect circular vouching patterns
fn detect_circular_vouch(
    env: &Env,
    voucher: Address,
    borrower: Address,
    current_depth: u32,
    visited: &mut Vec<Address>,
) -> bool {
    // Prevent infinite recursion by checking depth limit
    if current_depth > MAX_VOUCH_DEPTH {
        return true; // Depth exceeded, treat as circular
    }

    // If we've seen this address before in the current path, we have a cycle
    for v in visited.iter() {
        if v == borrower {
            return true;
        }
    }

    // Add borrower to visited path
    visited.push_back(borrower.clone());

    // Check if borrower has vouched for anyone in the current path
    let borrower_vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    for bv in borrower_vouches.iter() {
        // Check if the person borrower vouched for has vouched for our voucher
        let mut new_visited = visited.clone();
        if bv.voucher == voucher {
            return true; // Circular: voucher -> borrower -> voucher
        }
        // Recursively check deeper connections
        if detect_circular_vouch(env, voucher.clone(), bv.voucher.clone(), current_depth + 1, &mut new_visited) {
            return true;
        }
    }

    // Remove borrower from visited path (backtrack)
    if let Some(idx) = visited.iter().position(|a| a == borrower) {
        visited.remove(idx as u32);
    }

    false
}

// Task 3: Store vouch graph for circular detection
fn record_vouch_graph(env: &Env, voucher: Address, borrower: Address) {
    // Store the vouch relationship in the graph
    // Depth 1 means direct vouch
    env.storage()
        .persistent()
        .set(&DataKey::VouchGraph(voucher.clone(), borrower.clone()), &1u32);
    extend_ttl(env, &DataKey::VouchGraph(voucher, borrower));
}

pub fn vouch(
    env: Env,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;
    require_not_paused_for(&env, PauseFlag::Vouch)?;

    let whitelist_enabled: bool = env
        .storage()
        .instance()
        .get(&DataKey::VoucherWhitelistEnabled)
        .unwrap_or(false);
    if whitelist_enabled {
        let whitelisted: bool = env
            .storage()
            .persistent()
            .get(&DataKey::VoucherWhitelist(voucher.clone()))
            .unwrap_or(false);
        if !whitelisted {
            return Err(ContractError::VoucherNotWhitelisted);
        }
    }

    let sector = soroban_sdk::String::from_str(&env, "");
    check_and_update_cooldown(&env, &voucher)?;
    do_vouch(&env, voucher, borrower, stake, token, sector)
}

// #642: vouch with explicit sector for diversification enforcement
pub fn vouch_with_sector(
    env: Env,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    sector: soroban_sdk::String,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;
    require_not_paused_for(&env, PauseFlag::Vouch)?;

    let whitelist_enabled: bool = env
        .storage()
        .instance()
        .get(&DataKey::VoucherWhitelistEnabled)
        .unwrap_or(false);
    if whitelist_enabled {
        let whitelisted: bool = env
            .storage()
            .persistent()
            .get(&DataKey::VoucherWhitelist(voucher.clone()))
            .unwrap_or(false);
        if !whitelisted {
            return Err(ContractError::VoucherNotWhitelisted);
        }
    }

    check_and_update_cooldown(&env, &voucher)?;
    do_vouch(&env, voucher, borrower, stake, token, sector)
}

/// Check and enforce the per-voucher global cooldown.
/// Returns `VouchCooldownActive` if the cooldown has not elapsed.
/// Call this once per transaction (not per borrower in a batch).
fn check_and_update_cooldown(env: &Env, voucher: &Address) -> Result<(), ContractError> {
    let now = env.ledger().timestamp();
    let last: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::LastVouchTimestamp(voucher.clone()))
        .unwrap_or(0);
    if last > 0 && now < last + crate::types::DEFAULT_VOUCH_COOLDOWN_SECS {
        return Err(ContractError::VouchCooldownActive);
    }
    env.storage()
        .persistent()
        .set(&DataKey::LastVouchTimestamp(voucher.clone()), &now);
    extend_ttl(env, &DataKey::LastVouchTimestamp(voucher.clone()));
    Ok(())
}

fn do_vouch(
    env: &Env,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    sector: soroban_sdk::String,
) -> Result<(), ContractError> {
    // Validate numeric input: stake must be strictly positive.
    require_positive_amount(env, stake)?;

    assert!(voucher != borrower, "voucher cannot vouch for self");
    assert!(stake > 0, "stake must be greater than zero");

    // Validate token is allowed.
    let token_client = require_allowed_token(env, &token)?;

    // Sybil resistance: enforce minimum stake per vouch.
    let min_stake: i128 = env
        .storage()
        .instance()
        .get(&DataKey::MinStake)
        .unwrap_or(0);
    if min_stake > 0 && stake < min_stake {
        return Err(ContractError::MinStakeNotMet);
    }

    // Enforce per-voucher-per-borrower stake limit if set.
    if let Some(limit) = env
        .storage()
        .persistent()
        .get::<DataKey, i128>(&DataKey::VoucherStakeLimit(voucher.clone(), borrower.clone()))
    {
        if stake > limit {
            return Err(ContractError::StakeLimitExceeded);
        }
    }

    // NOTE: cooldown is enforced by the caller (vouch / batch_vouch) once per
    // transaction, not here, so that batch_vouch cannot bypass it by resetting
    // the timestamp on every iteration within the same ledger.

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    // Reject duplicate vouch (same voucher + same token) before any state mutation or transfer.
    for v in vouches.iter() {
        if v.voucher == voucher && v.token == token {
            return Err(ContractError::DuplicateVouch);
        }
    }

    // Reject vouch if the borrower already has an active loan — the stake
    // would be locked with no effect on the existing loan (fixes issue #13).
    if has_active_loan(env, &borrower) {
        return Err(ContractError::ActiveLoanExists);
    }

    // Task 3: Detect circular vouching patterns before processing
    let mut visited = Vec::new(env);
    if detect_circular_vouch(env, voucher.clone(), borrower.clone(), 1, &mut visited) {
        panic_with_error!(env, ContractError::CircularVouchDetected);
    }

    // Transfer stake from voucher into the contract.
    token_client.transfer(&voucher, &env.current_contract_address(), &stake);

    // Track voucher → borrowers history.
    let mut history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher.clone()))
        .unwrap_or(Vec::new(env));
    history.push_back(borrower.clone());
    env.storage()
        .persistent()
        .set(&DataKey::VoucherHistory(voucher.clone()), &history);
    extend_ttl(env, &DataKey::VoucherHistory(voucher.clone()));

    vouches.push_back(VouchRecord {
        voucher: voucher.clone(),
        amount: stake,
        vouch_timestamp: env.ledger().timestamp(),
        token: token.clone(),
        sector: sector.clone(),
    });
    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);
    extend_ttl(env, &DataKey::Vouches(borrower.clone()));

    // Task 3: Record vouch in the graph for circular detection
    record_vouch_graph(env, voucher.clone(), borrower.clone());

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("added")),
        (voucher, borrower, stake, token),
    );

    Ok(())
}

pub fn batch_vouch(
    env: Env,
    voucher: Address,
    borrowers: Vec<Address>,
    stakes: Vec<i128>,
    token: Address,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;
    require_not_paused_for(&env, PauseFlag::Vouch)?;

    assert!(borrowers.len() == stakes.len(), "borrowers and stakes length mismatch");
    assert!(!borrowers.is_empty(), "batch cannot be empty");

    // Enforce global cooldown once for the entire batch — prevents bypass by
    // spreading vouches across multiple borrowers in a single transaction.
    check_and_update_cooldown(&env, &voucher)?;

    for i in 0..borrowers.len() {
        let borrower = borrowers.get(i).unwrap();
        let stake = stakes.get(i).unwrap();
        let sector = soroban_sdk::String::from_str(&env, "");
        do_vouch(&env, voucher.clone(), borrower, stake, token.clone(), sector)?;
    }

    Ok(())
}

pub fn increase_stake(
    env: Env,
    voucher: Address,
    borrower: Address,
    additional: i128,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;
    // Task 1: Check granular pause for vouch operations
    require_not_paused_for(&env, PauseFlag::Vouch)?;

    require_positive_amount(&env, additional)?;

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .expect("vouch not found");

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .expect("vouch not found") as u32;

    let mut vouch_rec = vouches.get(idx).unwrap();
    // Use the token stored on the vouch record.
    let token_client = require_allowed_token(&env, &vouch_rec.token)?;

    // Enforce per-voucher-per-borrower stake limit if set.
    if let Some(limit) = env
        .storage()
        .persistent()
        .get::<DataKey, i128>(&DataKey::VoucherStakeLimit(voucher.clone(), borrower.clone()))
    {
        if vouch_rec.amount + additional > limit {
            return Err(ContractError::StakeLimitExceeded);
        }
    }

    token_client.transfer(&voucher, &env.current_contract_address(), &additional);

    vouch_rec.amount += additional;
    vouches.set(idx, vouch_rec);

    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);
    extend_ttl(&env, &DataKey::Vouches(borrower));

    Ok(())
}

pub fn decrease_stake(
    env: Env,
    voucher: Address,
    borrower: Address,
    amount: i128,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;
    // Task 1: Check granular pause for withdraw operations
    require_not_paused_for(&env, PauseFlag::Withdraw)?;

    if voucher == borrower {
        return Err(ContractError::SelfVouchNotAllowed);
    }
    assert!(amount > 0, "decrease amount must be greater than zero");
    assert!(!has_active_loan(&env, &borrower), "loan already active");

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .expect("vouch not found");

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .expect("vouch not found") as u32;

    let mut vouch_rec = vouches.get(idx).unwrap();
    assert!(
        amount <= vouch_rec.amount,
        "decrease amount exceeds staked amount"
    );

    let token_client = require_allowed_token(&env, &vouch_rec.token)?;
    vouch_rec.amount -= amount;
    if vouch_rec.amount == 0 {
        vouches.remove(idx);
    } else {
        vouches.set(idx, vouch_rec);
    }

    if vouches.is_empty() {
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower));
    } else {
        env.storage()
            .persistent()
            .set(&DataKey::Vouches(borrower), &vouches);
    }

    token_client.transfer(&env.current_contract_address(), &voucher, &amount);

    Ok(())
}

pub fn withdraw_vouch(env: Env, voucher: Address, borrower: Address) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;
    // Task 1: Check granular pause for withdraw operations
    require_not_paused_for(&env, PauseFlag::Withdraw)?;

    assert!(!has_active_loan(&env, &borrower), "loan already active");

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoActiveLoan)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .ok_or(ContractError::UnauthorizedCaller)? as u32;

    let vouch_rec = vouches.get(idx).unwrap();
    let stake = vouch_rec.amount;
    let token_addr = vouch_rec.token.clone();
    vouches.remove(idx);

    if vouches.is_empty() {
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));
    } else {
        env.storage()
            .persistent()
            .set(&DataKey::Vouches(borrower.clone()), &vouches);
    }

    let token_client = require_allowed_token(&env, &token_addr)?;
    token_client.transfer(&env.current_contract_address(), &voucher, &stake);

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("withdrawn")),
        (voucher, borrower, stake),
    );

    Ok(())
}

pub fn transfer_vouch(
    env: Env,
    from: Address,
    to: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    from.require_auth();
    require_not_paused(&env)?;
    // Task 1: Check granular pause for vouch operations
    require_not_paused_for(&env, PauseFlag::Vouch)?;

    if from == to {
        return Ok(());
    }

    // Only allow transfer before a loan is active (consistent with withdraw_vouch).
    assert!(!has_active_loan(&env, &borrower), "loan already active");

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoActiveLoan)?;

    let from_idx = vouches
        .iter()
        .position(|v| v.voucher == from)
        .ok_or(ContractError::UnauthorizedCaller)? as u32;

    let from_record = vouches.get(from_idx).unwrap();
    let stake_to_transfer = from_record.amount;

    if let Some(to_idx) = vouches.iter().position(|v| v.voucher == to) {
        // Merge into existing record for 'to'
        let mut to_record = vouches.get(to_idx as u32).unwrap();
        to_record.amount += stake_to_transfer;
        vouches.set(to_idx as u32, to_record);
        vouches.remove(from_idx);
    } else {
        // Transfer ownership to 'to'
        let mut updated_record = from_record;
        updated_record.voucher = to.clone();
        vouches.set(from_idx, updated_record);
    }

    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);
    extend_ttl(&env, &DataKey::Vouches(borrower.clone()));

    // Update voucher histories
    // 1. Remove borrower from 'from' history
    let mut from_history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(from.clone()))
        .unwrap_or(Vec::new(&env));
    if let Some(h_idx) = from_history.iter().position(|b| b == borrower) {
        from_history.remove(h_idx as u32);
        env.storage()
            .persistent()
            .set(&DataKey::VoucherHistory(from.clone()), &from_history);
        extend_ttl(&env, &DataKey::VoucherHistory(from.clone()));
    }

    // 2. Add borrower to 'to' history if not already there
    let mut to_history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(to.clone()))
        .unwrap_or(Vec::new(&env));
    if !to_history.iter().any(|b| b == borrower) {
        to_history.push_back(borrower.clone());
        env.storage()
            .persistent()
            .set(&DataKey::VoucherHistory(to.clone()), &to_history);
        extend_ttl(&env, &DataKey::VoucherHistory(to.clone()));
    }

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("transfer")),
        (from, to, borrower, stake_to_transfer),
    );

    Ok(())
}

pub fn vouch_exists(env: Env, voucher: Address, borrower: Address) -> bool {
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));
    vouches.iter().any(|v| v.voucher == voucher)
}

pub fn total_vouched(env: Env, borrower: Address) -> Result<i128, ContractError> {
    let vouches = env
        .storage()
        .persistent()
        .get::<DataKey, Vec<VouchRecord>>(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));

    let mut total: i128 = 0;
    for vouch in vouches.iter() {
        total = total
            .checked_add(vouch.amount)
            .ok_or(ContractError::StakeOverflow)?;
    }

    Ok(total)
}

pub fn voucher_history(env: Env, voucher: Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher))
        .unwrap_or(Vec::new(&env))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DataKey;
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn create_test_token(env: &Env) -> Address {
        let admin = Address::generate(env);
        env.register_stellar_asset_contract_v2(admin).address()
    }

    fn create_test_admin(env: &Env) -> Address {
        Address::generate(env)
    }

    #[test]
    fn test_total_vouched_overflow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);
        let admin = create_test_admin(&env);
        let admins = Vec::from_array(&env, [admin]);
        let token = create_test_token(&env);

        client.initialize(&deployer, &admins, &1, &token);

        let borrower = Address::generate(&env);

        // Create vouches that would overflow when summed
        let mut vouches = Vec::new(&env);

        // Add two vouches with very large stakes that would overflow i128::MAX
        let voucher1 = Address::generate(&env);
        let voucher2 = Address::generate(&env);

        vouches.push_back(VouchRecord {
            voucher: voucher1,
            amount: i128::MAX - 1000,
            vouch_timestamp: 0,
            token: token.clone(),
            sector: soroban_sdk::String::from_str(&env, "general"),
        });

        vouches.push_back(VouchRecord {
            voucher: voucher2,
            amount: 2000, // This would cause overflow when added to the first stake
            vouch_timestamp: 0,
            token: token.clone(),
            sector: soroban_sdk::String::from_str(&env, "general"),
        });

        // Store the vouches directly in contract storage
        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Vouches(borrower.clone()), &vouches);
        });

        // Test that total_vouched returns StakeOverflow error
        let result = client.try_total_vouched(&borrower);
        assert_eq!(result, Err(Ok(ContractError::StakeOverflow)));
    }

    #[test]
    fn test_total_vouched_no_overflow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);
        let admin = create_test_admin(&env);
        let admins = Vec::from_array(&env, [admin]);
        let token = create_test_token(&env);

        client.initialize(&deployer, &admins, &1, &token);

        let borrower = Address::generate(&env);

        // Create vouches with normal stakes that won't overflow
        let mut vouches = Vec::new(&env);

        let voucher1 = Address::generate(&env);
        let voucher2 = Address::generate(&env);

        vouches.push_back(VouchRecord {
            voucher: voucher1,
            amount: 1_000_000,
            vouch_timestamp: 0,
            token: token.clone(),
            sector: soroban_sdk::String::from_str(&env, "general"),
        });

        vouches.push_back(VouchRecord {
            voucher: voucher2,
            amount: 2_500_000,
            vouch_timestamp: 0,
            token: token.clone(),
            sector: soroban_sdk::String::from_str(&env, "general"),
        });

        // Store the vouches directly in contract storage
        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&DataKey::Vouches(borrower.clone()), &vouches);
        });

        // Test that total_vouched returns correct sum
        let result = client.total_vouched(&borrower);
        assert_eq!(result, 3_500_000);
    }

    /// Issue #442: decrease_stake() must reject self-vouch (voucher == borrower)
    #[test]
    fn test_decrease_stake_self_vouch_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);
        let admin = create_test_admin(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);
        let token = create_test_token(&env);

        client.initialize(&deployer, &admins, &1, &token);

        let user = Address::generate(&env);

        let result = client.try_decrease_stake(&user, &user, &1_000);
        assert_eq!(result, Err(Ok(ContractError::SelfVouchNotAllowed)));
    }
}
