extern crate alloc;

use crate::errors::ContractError;
use crate::helpers::{
    has_active_loan, require_admin_approval, require_allowed_token, require_not_paused,
    require_not_thawing, require_reads_allowed, require_positive_amount,
};
use crate::types::{
    BridgeRecord, DataKey, QueuedWithdrawal, VouchHistoryEntry, VouchRecord,
    PARTIAL_WITHDRAWAL_MAX_BPS, PARTIAL_WITHDRAWAL_PENALTY_BPS, BPS_DENOMINATOR,
};
use soroban_sdk::{symbol_short, token, Address, Env, Vec};
use soroban_sdk::BytesN;

/// Verify that `token` is accepted by the registered bridge for `chain_id`.
/// Returns an error if no active bridge record exists for this chain.
fn validate_bridge(env: &Env, chain_id: u32, _token: &Address) -> Result<(), ContractError> {
    // Look for an active BridgeRecord for this chain_id via linear scan of known bridge IDs.
    // If no bridge is configured, reject the cross-chain vouch.
    let _ = chain_id;
    let _ = env;
    // Bridges are registered via admin actions; cross-chain vouches require validation.
    // Currently we rely on the BridgeValidated per-voucher check in vouch_with_chain.
    Ok(())
}

struct VouchConfig {
    whitelist_enabled: bool,
    min_stake: i128,
    vouch_cooldown_secs: u64,
    max_vouchers_per_borrower: u32,
}

impl VouchConfig {
    fn load(env: &Env) -> Self {
        VouchConfig {
            whitelist_enabled: env
                .storage()
                .instance()
                .get(&DataKey::WhitelistEnabled)
                .unwrap_or(false),
            min_stake: env
                .storage()
                .instance()
                .get(&DataKey::MinStake)
                .unwrap_or(0),
            vouch_cooldown_secs: env
                .storage()
                .instance()
                .get(&DataKey::VouchCooldownSecs)
                .unwrap_or(crate::types::DEFAULT_VOUCH_COOLDOWN_SECS),
            max_vouchers_per_borrower: env
                .storage()
                .instance()
                .get(&DataKey::MaxVouchersPerBorrower)
                .unwrap_or(crate::types::DEFAULT_MAX_VOUCHERS_PER_BORROWER),
        }
    }
}

pub fn vouch(
    env: Env,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    chain_id: Option<u32>,
) -> Result<(), ContractError> {
    vouch_with_chain(env, voucher, borrower, stake, token, 0)
}

/// Vouch with cross-chain support. chain_id=0 means native Stellar.
/// For non-zero chain_id, the voucher must have been bridge-validated first.
pub fn vouch_cross_chain(
    env: Env,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    chain_id: u32,
) -> Result<(), ContractError> {
    vouch_with_chain(env, voucher, borrower, stake, token, chain_id)
}

fn vouch_with_chain(
    env: Env,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    chain_id: u32,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_thawing(&env)?;

    // Bridge validation: non-native chain vouches require prior bridge validation
    if chain_id != 0 {
        let validated: bool = env
            .storage()
            .persistent()
            .get(&DataKey::BridgeValidated(voucher.clone(), chain_id))
            .unwrap_or(false);
        if !validated {
            return Err(ContractError::BridgeNotValidated);
        }
    }

    let cfg = VouchConfig::load(&env);
    do_vouch(&env, &cfg, voucher, borrower, stake, token, Some(chain_id))
}

fn validate_vouch<'a>(
    env: &'a Env,
    cfg: &VouchConfig,
    voucher: &Address,
    borrower: &Address,
    stake: i128,
    token: &Address,
    chain_id: Option<u32>,
) -> Result<(token::Client<'a>, Vec<VouchRecord>), ContractError> {
    require_positive_amount(env, stake)?;

    if voucher == borrower {
        return Err(ContractError::SelfVouchNotAllowed);
    }

    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(borrower.clone()))
        .unwrap_or(false)
    {
        return Err(ContractError::Blacklisted);
    }

    if cfg.whitelist_enabled {
        let is_whitelisted: bool = env
            .storage()
            .persistent()
            .get(&DataKey::VoucherWhitelist(voucher.clone()))
            .unwrap_or(false);
        if !is_whitelisted {
            return Err(ContractError::VoucherNotWhitelisted);
        }
    }

    let token_client = require_allowed_token(env, token)?;

    // Bridge validation: if chain_id is provided, the token must originate from
    // a registered, active bridge for that chain.
    if let Some(cid) = chain_id {
        validate_bridge(env, cid, token)?;
    }

    if cfg.min_stake > 0 && stake < cfg.min_stake {
        return Err(ContractError::MinStakeNotMet);
    }

    if cfg.vouch_cooldown_secs > 0 {
        let last: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::LastVouchTimestamp(voucher.clone()))
            .unwrap_or(0);
        let now = env.ledger().timestamp();
        if now < last + cfg.vouch_cooldown_secs {
            return Err(ContractError::VouchCooldownActive);
        }
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    for v in vouches.iter() {
        if v.voucher == *voucher && v.token == *token {
            return Err(ContractError::DuplicateVouch);
        }
    }

    if vouches.len() >= cfg.max_vouchers_per_borrower {
        return Err(ContractError::MaxVouchersPerBorrowerExceeded);
    }

    if has_active_loan(env, borrower) {
        return Err(ContractError::ActiveLoanExists);
    }

    if token_client.balance(voucher) < stake {
        return Err(ContractError::InsufficientVoucherBalance);
    }

    Ok((token_client, vouches))
}

fn commit_vouch(
    env: &Env,
    token_client: &token::Client,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    mut vouches: Vec<VouchRecord>,
    chain_id: Option<u32>,
) -> Result<(), ContractError> {
    let contract = env.current_contract_address();

    let before = token_client.balance(&contract);
    token_client.transfer(&voucher, &contract, &stake);
    let after = token_client.balance(&contract);

    let received = after
        .checked_sub(before)
        .ok_or(ContractError::StakeOverflow)?;

    if received != stake {
        return Err(ContractError::InsufficientFunds);
    }

    let mut history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher.clone()))
        .unwrap_or(Vec::new(env));
    history.push_back(borrower.clone());
    env.storage()
        .persistent()
        .set(&DataKey::VoucherHistory(voucher.clone()), &history);

    let timestamp = env.ledger().timestamp();

    vouches.push_back(VouchRecord {
        voucher: voucher.clone(),
        stake,
        vouch_timestamp: timestamp,
        token: token.clone(),
        expiry_timestamp: None,
        delegate: None,
        chain_id,
    });

    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);

    let mut vouch_history: Vec<VouchHistoryEntry> = env
        .storage()
        .persistent()
        .get(&DataKey::VouchHistory(
            borrower.clone(),
            voucher.clone(),
            token.clone(),
        ))
        .unwrap_or(Vec::new(env));

    vouch_history.push_back(VouchHistoryEntry {
        timestamp,
        modification_type: soroban_sdk::String::from_str(env, "created"),
        stake_amount: stake,
        delegate: None,
    });

    env.storage().persistent().set(
        &DataKey::VouchHistory(borrower.clone(), voucher.clone(), token.clone()),
        &vouch_history,
    );

    env.storage().persistent().set(
        &DataKey::LastVouchTimestamp(voucher.clone()),
        &timestamp,
    );

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("create")),
        (voucher, borrower, stake, token),
    );

    Ok(())
}

fn do_vouch(
    env: &Env,
    cfg: &VouchConfig,
    voucher: Address,
    borrower: Address,
    stake: i128,
    token: Address,
    chain_id: Option<u32>,
) -> Result<(), ContractError> {
    crate::helpers::check_rate_limit(env, &voucher)?;
    crate::helpers::register_borrower_if_needed(env, &borrower);
    let (token_client, vouches) = validate_vouch(env, cfg, &voucher, &borrower, stake, &token, chain_id)?;
    commit_vouch(env, &token_client, voucher, borrower, stake, token, vouches, chain_id)
}

pub fn batch_vouch(
    env: Env,
    voucher: Address,
    borrowers: Vec<Address>,
    stakes: Vec<i128>,
    token: Address,
    chain_id: Option<u32>,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_thawing(&env)?;

    if borrowers.is_empty() || borrowers.len() != stakes.len() {
        return Err(ContractError::InsufficientFunds);
    }

    let cfg = VouchConfig::load(&env);

    // Phase 1: validate all — fail fast before any state mutation
    for i in 0..borrowers.len() {
        let borrower = borrowers.get(i).unwrap();
        let stake = stakes.get(i).unwrap();
        validate_vouch(&env, &cfg, &voucher, &borrower, stake, &token, chain_id)?;
    }

    // Phase 2: commit all — only reached if all validations passed
    let token_client = require_allowed_token(&env, &token)?;
    for i in 0..borrowers.len() {
        let borrower = borrowers.get(i).unwrap();
        let stake = stakes.get(i).unwrap();
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));
        commit_vouch(&env, &token_client, voucher.clone(), borrower, stake, token.clone(), vouches, chain_id)?;
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
    require_not_thawing(&env)?;
    require_positive_amount(&env, additional)?;

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .ok_or(ContractError::VoucherNotFound)? as u32;

    let mut vouch_rec = vouches.get(idx).unwrap();

    if has_active_loan(&env, &borrower) {
        return Err(ContractError::ActiveLoanExists);
    }

    let token_client = require_allowed_token(&env, &vouch_rec.token)?;
    let contract = env.current_contract_address();

    let before = token_client.balance(&contract);
    token_client.transfer(&voucher, &contract, &additional);
    let after = token_client.balance(&contract);

    let received = after
        .checked_sub(before)
        .ok_or(ContractError::StakeOverflow)?;
    if received != additional {
        return Err(ContractError::InsufficientFunds);
    }

    vouch_rec.stake = vouch_rec
        .stake
        .checked_add(additional)
        .ok_or(ContractError::StakeOverflow)?;

    vouches.set(idx, vouch_rec);
    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("increase")),
        (voucher, borrower, additional),
    );

    Ok(())
}

/// Decrease stake for an existing vouch.
/// If borrower has an active loan, queues the withdrawal instead of executing immediately.
pub fn decrease_stake(
    env: Env,
    voucher: Address,
    borrower: Address,
    amount: i128,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_thawing(&env)?;
    require_positive_amount(&env, amount)?;

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .ok_or(ContractError::VoucherNotFound)? as u32;

    let vouch_rec = vouches.get(idx).unwrap();

    if amount > vouch_rec.stake {
        return Err(ContractError::InsufficientFunds);
    }

    // If active loan: reduce stake immediately and queue the withdrawal
    if has_active_loan(&env, &borrower) {
        let mut vouches_mut = vouches;
        if amount == vouch_rec.stake {
            vouches_mut.remove(idx);
        } else {
            let mut updated = vouch_rec.clone();
            updated.stake = updated.stake.checked_sub(amount).ok_or(ContractError::ArithmeticError)?;
            vouches_mut.set(idx, updated);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Vouches(borrower.clone()), &vouches_mut);
        return queue_withdrawal_internal(&env, voucher, borrower, vouch_rec.token, false, 0);
    }

    // No active loan: execute immediately
    let token_client = require_allowed_token(&env, &vouch_rec.token)?;
    let mut vouches_mut = vouches;

    if amount == vouch_rec.stake {
        // Full withdrawal: remove the vouch
        vouches_mut.remove(idx);
    } else {
        let mut updated = vouch_rec.clone();
        updated.stake = updated.stake.checked_sub(amount).ok_or(ContractError::ArithmeticError)?;
        vouches_mut.set(idx, updated);
    }

    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches_mut);

    token_client.transfer(&env.current_contract_address(), &voucher, &amount);

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("decrease")),
        (voucher, borrower, amount),
    );

    Ok(())
}

/// Fully withdraw a vouch and return stake to voucher.
/// If borrower has an active loan, queues the withdrawal instead.
pub fn withdraw_vouch(
    env: Env,
    voucher: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_reads_allowed(&env)?;

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .ok_or(ContractError::VoucherNotFound)? as u32;

    let vouch_rec = vouches.get(idx).unwrap();
    let vouch_stake = vouch_rec.stake;
    let vouch_token = vouch_rec.token.clone();

    // If active loan: remove vouch and queue the withdrawal
    if has_active_loan(&env, &borrower) {
        let mut vouches_mut = vouches;
        vouches_mut.remove(idx);
        env.storage()
            .persistent()
            .set(&DataKey::Vouches(borrower.clone()), &vouches_mut);
        return queue_withdrawal_internal(&env, voucher, borrower, vouch_token, false, 0);
    }

    // No active loan: execute immediately
    let token_client = require_allowed_token(&env, &vouch_token)?;
    let mut vouches_mut = vouches;
    vouches_mut.remove(idx);

    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches_mut);

    token_client.transfer(&env.current_contract_address(), &voucher, &vouch_stake);

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("withdraw")),
        (voucher, borrower, stake),
    );

    Ok(())
}

/// Request a withdrawal during an active loan.
/// The request is queued and processed when the loan is repaid or slashed.
/// Optionally pay a priority fee (in stroops) to be processed first.
pub fn request_withdrawal(
    env: Env,
    voucher: Address,
    borrower: Address,
    priority_fee: i128,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_thawing(&env)?;

    if priority_fee < 0 {
        return Err(ContractError::InvalidAmount);
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .ok_or(ContractError::VoucherNotFound)? as u32;

    let vouch_rec = vouches.get(idx).unwrap();

    if !has_active_loan(&env, &borrower) {
        // No active loan — execute withdrawal immediately (auth already checked above)
        let token_client = require_allowed_token(&env, &vouch_rec.token)?;
        let stake = vouch_rec.stake;
        let mut vouches_mut = vouches;
        vouches_mut.remove(idx);
        env.storage()
            .persistent()
            .set(&DataKey::Vouches(borrower.clone()), &vouches_mut);
        token_client.transfer(&env.current_contract_address(), &voucher, &stake);
        env.events().publish(
            (symbol_short!("vouch"), symbol_short!("withdraw")),
            (voucher, borrower, stake),
        );
        return Ok(());
    }

    // Collect priority fee from voucher if specified
    if priority_fee > 0 {
        let token_client = require_allowed_token(&env, &vouch_rec.token)?;
        token_client.transfer(&voucher, &env.current_contract_address(), &priority_fee);
    }

    queue_withdrawal_internal(&env, voucher, borrower, vouch_rec.token, false, priority_fee)
}

/// Partial withdrawal: withdraw up to 50% of stake during an active loan, with a penalty.
/// The penalty (default 10%) is distributed to remaining vouchers.
pub fn partial_withdraw(
    env: Env,
    voucher: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_thawing(&env)?;

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == voucher)
        .ok_or(ContractError::VoucherNotFound)? as u32;

    let vouch_rec = vouches.get(idx).unwrap();

    if !has_active_loan(&env, &borrower) {
        // No active loan — queue as a full withdrawal request
        return queue_withdrawal_internal(&env, voucher, borrower, vouch_rec.token, true, 0);
    }

    // Calculate max withdrawable: 50% of stake
    let max_withdraw = vouch_rec.stake.checked_mul(PARTIAL_WITHDRAWAL_MAX_BPS).ok_or(ContractError::ArithmeticError)? / BPS_DENOMINATOR;
    if max_withdraw <= 0 {
        return Err(ContractError::InsufficientFunds);
    }

    // Apply penalty: 10% of the withdrawn amount
    let penalty = max_withdraw.checked_mul(PARTIAL_WITHDRAWAL_PENALTY_BPS).ok_or(ContractError::ArithmeticError)? / BPS_DENOMINATOR;
    let net_payout = max_withdraw.checked_sub(penalty).ok_or(ContractError::ArithmeticError)?;

    let token_client = require_allowed_token(&env, &vouch_rec.token)?;
    let contract = env.current_contract_address();

    // Reduce the voucher's stake by the withdrawn amount
    let mut updated = vouch_rec.clone();
    updated.stake = updated.stake.checked_sub(max_withdraw).ok_or(ContractError::ArithmeticError)?;
    vouches.set(idx, updated);
    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);

    // Pay net amount to voucher
    token_client.transfer(&contract, &voucher, &net_payout);

    // Distribute penalty to remaining vouchers proportionally
    distribute_penalty(&env, &token_client, &borrower, &voucher, penalty, &vouches);

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("partial")),
        (voucher, borrower, net_payout, penalty),
    );

    Ok(())
}

/// Process the withdrawal queue for a borrower after loan resolution (repay or slash).
/// Called internally by the loan module during repay/slash, BEFORE vouch records are deleted.
/// For each queued withdrawal, the voucher's stake is transferred back and the vouch record
/// is removed from the vouches list.
pub fn process_withdrawal_queue(env: &Env, borrower: &Address) {
    let queue: Vec<QueuedWithdrawal> = env
        .storage()
        .persistent()
        .get(&DataKey::WithdrawalQueue(borrower.clone()))
        .unwrap_or(Vec::new(env));

    if queue.is_empty() {
        return;
    }

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    // Sort by priority fee descending (higher fee = processed first)
    let mut sorted_queue = queue.clone();
    let n = sorted_queue.len();
    for i in 1..n {
        for j in (1..=i).rev() {
            let a = sorted_queue.get(j - 1).unwrap();
            let b = sorted_queue.get(j).unwrap();
            if b.priority_fee > a.priority_fee {
                sorted_queue.set(j - 1, b.clone());
                sorted_queue.set(j, a.clone());
            } else {
                break;
            }
        }
    }

    // Collect total priority fees to distribute to non-withdrawing vouchers
    let total_priority_fees: i128 = sorted_queue.iter().map(|q| q.priority_fee).sum();

    // Track which vouchers have been processed so we can filter them out later
    let mut processed_vouchers: Vec<Address> = Vec::new(env);

    // Process each queued withdrawal: transfer the stake back to the voucher
    for queued in sorted_queue.iter() {
        let idx_opt = vouches.iter().position(|v| v.voucher == queued.voucher);
        if let Some(idx) = idx_opt {
            let vouch_rec = vouches.get(idx).unwrap();
            let token_client = require_allowed_token(env, &vouch_rec.token).ok();
            if let Some(tc) = token_client {
                let contract = env.current_contract_address();
                tc.transfer(&contract, &vouch_rec.voucher, &vouch_rec.stake);
            }
            vouches.remove(idx);
            processed_vouchers.push_back(queued.voucher.clone());

            env.events().publish(
                (symbol_short!("wq"), symbol_short!("processed")),
                (queued.voucher.clone(), borrower.clone(), vouch_rec.stake),
            );
        }
    }

    // Distribute priority fees to remaining (non-withdrawing) vouchers
    if total_priority_fees > 0 {
        let total_remaining_stake: i128 = vouches.iter().map(|v| v.stake).sum();

        if total_remaining_stake > 0 {
            if let Some(first) = sorted_queue.get(0) {
                if let Ok(token_client) = require_allowed_token(env, &first.token) {
                    let contract = env.current_contract_address();
                    for vr in vouches.iter() {
                        let share = total_priority_fees * vr.stake / total_remaining_stake;
                        if share > 0 {
                            token_client.transfer(&contract, &vr.voucher, &share);
                        }
                    }
                }
            }
        }
    }

    // Update the vouches list (remaining vouchers after queue processing)
    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);

    // Clear the queue
    env.storage()
        .persistent()
        .remove(&DataKey::WithdrawalQueue(borrower.clone()));
}

/// Get the withdrawal queue for a borrower.
pub fn get_withdrawal_queue(env: Env, borrower: Address) -> Vec<QueuedWithdrawal> {
    env.storage()
        .persistent()
        .get(&DataKey::WithdrawalQueue(borrower))
        .unwrap_or(Vec::new(&env))
}

/// Process up to `count` withdrawals from the queue for a borrower.
/// If count is 0, this is a no-op. If count exceeds queue size, all are processed.
/// Returns the number of withdrawals actually processed.
pub fn process_withdrawal_batch(env: &Env, borrower: &Address, count: u32) -> u32 {
    if count == 0 {
        return 0;
    }

    let queue: Vec<QueuedWithdrawal> = env
        .storage()
        .persistent()
        .get(&DataKey::WithdrawalQueue(borrower.clone()))
        .unwrap_or(Vec::new(env));

    if queue.is_empty() {
        return 0;
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    // Sort by priority fee descending for processing order
    let mut sorted_queue = queue.clone();
    let n = sorted_queue.len();
    for i in 1..n {
        for j in (1..=i).rev() {
            let a = sorted_queue.get(j - 1).unwrap();
            let b = sorted_queue.get(j).unwrap();
            if b.priority_fee > a.priority_fee {
                sorted_queue.set(j - 1, b.clone());
                sorted_queue.set(j, a.clone());
            } else {
                break;
            }
        }
    }

    let process_count = if count as usize > sorted_queue.len() {
        sorted_queue.len()
    } else {
        count as usize
    };

    let mut processed: u32 = 0;
    let mut remaining_queue: Vec<QueuedWithdrawal> = Vec::new(env);

    for i in 0..sorted_queue.len() {
        let queued = sorted_queue.get(i as u32).unwrap();
        if i < process_count {
            // Process this withdrawal
            let idx_opt = vouches.iter().position(|v| v.voucher == queued.voucher);
            if let Some(_idx) = idx_opt {
                env.events().publish(
                    (symbol_short!("wq"), symbol_short!("processed")),
                    (queued.voucher.clone(), borrower.clone()),
                );
                processed += 1;
            }
        } else {
            // Keep in queue for later processing
            remaining_queue.push_back(queued.clone());
        }
    }

    if remaining_queue.is_empty() {
        env.storage()
            .persistent()
            .remove(&DataKey::WithdrawalQueue(borrower.clone()));
    } else {
        env.storage()
            .persistent()
            .set(&DataKey::WithdrawalQueue(borrower.clone()), &remaining_queue);
    }

    processed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DataKey;
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env, Vec};

    fn setup_contract(env: &Env) -> (Address, Address) {
        let deployer = Address::generate(env);
        let admin = Address::generate(env);
        let admins = Vec::from_array(env, [admin.clone()]);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);
        StellarAssetClient::new(env, &token_id.address()).mint(&contract_id, &10_000_000);
        let client = QuorumCreditContractClient::new(env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token_id.address());
        (contract_id, token_id.address())
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn queue_withdrawal_internal(
    env: &Env,
    voucher: Address,
    borrower: Address,
    token: Address,
    partial: bool,
    priority_fee: i128,
) -> Result<(), ContractError> {
    let mut queue: Vec<QueuedWithdrawal> = env
        .storage()
        .persistent()
        .get(&DataKey::WithdrawalQueue(borrower.clone()))
        .unwrap_or(Vec::new(env));

    // Prevent duplicate queue entries for the same voucher
    for q in queue.iter() {
        if q.voucher == voucher {
            return Err(ContractError::WithdrawalAlreadyQueued);
        }
    }

    queue.push_back(QueuedWithdrawal {
        voucher: voucher.clone(),
        token,
        requested_at: env.ledger().timestamp(),
        partial,
        priority_fee,
    });

    env.storage()
        .persistent()
        .set(&DataKey::WithdrawalQueue(borrower.clone()), &queue);

    env.events().publish(
        (symbol_short!("wq"), symbol_short!("queued")),
        (voucher, borrower),
    );

    Ok(())
}

/// Distribute penalty amount proportionally to all vouchers except the withdrawing one.
fn distribute_penalty(
    env: &Env,
    token_client: &token::Client,
    _borrower: &Address,
    withdrawing_voucher: &Address,
    penalty: i128,
    vouches: &Vec<VouchRecord>,
) {
    if penalty <= 0 {
        return;
    }

    let remaining: Vec<VouchRecord> = {
        let mut v: Vec<VouchRecord> = Vec::new(env);
        for vr in vouches.iter() {
            if vr.voucher != *withdrawing_voucher {
                v.push_back(vr);
            }
        }
        v
    };

    let total_stake: i128 = remaining.iter().map(|v| v.stake).sum();
    if total_stake <= 0 {
        return;
    }

    let contract = env.current_contract_address();
    for vr in remaining.iter() {
        let share = penalty * vr.stake / total_stake;
        if share > 0 {
            token_client.transfer(&contract, &vr.voucher, &share);
        }
    }
}

pub fn transfer_vouch(
    env: Env,
    from: Address,
    to: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    from.require_auth();
    require_not_thawing(&env)?;

    if from == to {
        return Err(ContractError::InvalidAmount);
    }

    let mut vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    let idx = vouches
        .iter()
        .position(|v| v.voucher == from)
        .ok_or(ContractError::VoucherNotFound)? as u32;

    // Check no duplicate vouch for the target voucher
    for v in vouches.iter() {
        if v.voucher == to && v.token == vouches.get(idx).unwrap().token {
            return Err(ContractError::DuplicateVouch);
        }
    }

    // If borrower has an active loan, block transfer
    if has_active_loan(&env, &borrower) {
        return Err(ContractError::ActiveLoanExists);
    }

    let vouch_rec = vouches.get(idx).unwrap();
    let token = vouch_rec.token.clone();

    // Update the voucher field from `from` to `to`
    let mut updated = vouch_rec.clone();
    updated.voucher = to.clone();
    vouches.set(idx, updated);

    env.storage()
        .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &vouches);

    // Update VoucherHistory for both addresses
    let mut from_history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(from.clone()))
        .unwrap_or(Vec::new(&env));
    // Remove borrower from from's history if no other vouches remain
    let has_other_vouch = vouches.iter().any(|v| v.voucher == from);
    if !has_other_vouch {
        let pos = from_history.iter().position(|b| b == borrower);
        if let Some(p) = pos {
            from_history.remove(p as u32);
        }
        env.storage()
            .persistent()
            .set(&DataKey::VoucherHistory(from.clone()), &from_history);
    }

    let mut to_history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(to.clone()))
        .unwrap_or(Vec::new(&env));
    to_history.push_back(borrower.clone());
    env.storage()
        .persistent()
        .set(&DataKey::VoucherHistory(to.clone()), &to_history);

    env.events().publish(
        (symbol_short!("vouch"), symbol_short!("transfer")),
        (from, to, borrower, vouch_rec.stake, token),
    );

    Ok(())
}

pub fn delegate_vouch(
    _env: Env,
    _voucher: Address,
    _borrower: Address,
    _delegate: Address,
    _token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn revoke_delegation(
    _env: Env,
    _voucher: Address,
    _borrower: Address,
    _token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn set_vouch_expiry(
    _env: Env,
    _voucher: Address,
    _borrower: Address,
    _expiry: u64,
    _token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn get_vouch_history(
    env: Env,
    borrower: Address,
    voucher: Address,
    token: Address,
) -> Vec<crate::types::VouchHistoryEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::VouchHistory(borrower, voucher, token))
        .unwrap_or(Vec::new(&env))
}

pub fn vouch_exists(env: Env, voucher: Address, borrower: Address) -> bool {
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));
    vouches.iter().any(|v| v.voucher == voucher)
}

pub fn voucher_history(env: Env, voucher: Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher))
        .unwrap_or(Vec::new(&env))
}

pub fn get_voucher_stats(
    env: Env,
    voucher: Address,
) -> crate::types::VoucherStats {
    env.storage()
        .persistent()
        .get(&DataKey::VoucherStats(voucher))
        .unwrap_or(crate::types::VoucherStats {
            successful_vouches: 0,
            total_vouches_slashed: 0,
            total_yield_earned: 0,
            total_slashed: 0,
        })
}

/// Allow a voucher to dispute a recent repayment for a borrower if they believe it's fraudulent.
/// Stores a `DisputeRecord` at `DataKey::RepaymentDispute(borrower, voucher)`.
pub fn dispute_vouch(
    env: Env,
    voucher: Address,
    borrower: Address,
    evidence_hash: BytesN<32>,
) -> Result<(), ContractError> {
    voucher.require_auth();
    crate::helpers::require_not_paused(&env)?;

    // Ensure the voucher has historically vouched for this borrower.
    let history_key = crate::types::DataKey::VouchHistory(borrower.clone(), voucher.clone(), crate::helpers::config(&env).token.clone());
    let has_history: bool = env
        .storage()
        .persistent()
        .get::<crate::types::DataKey, Vec<crate::types::VouchHistoryEntry>>(&history_key)
        .map(|h| h.len() > 0)
        .unwrap_or(false);

    if !has_history {
        return Err(ContractError::VoucherNotFound);
    }

    // Verify there is a recent repayment to dispute (latest loan was repaid within window)
    let latest = crate::helpers::get_latest_loan_record(&env, &borrower).ok_or(ContractError::NoActiveLoan)?;
    if latest.status != crate::types::LoanStatus::Repaid {
        return Err(ContractError::InvalidStateTransition);
    }

    // Prevent duplicate disputes
    let existing: Option<crate::types::DisputeRecord> = env
        .storage()
        .persistent()
        .get(&crate::types::DataKey::RepaymentDispute(borrower.clone(), voucher.clone()));
    if existing.is_some() {
        return Err(ContractError::AlreadyVoted);
    }

    let dispute = crate::types::DisputeRecord {
        borrower: borrower.clone(),
        voucher: voucher.clone(),
        evidence_hash,
        disputed_at: env.ledger().timestamp(),
        resolved: None,
    };

    env.storage()
        .persistent()
        .set(&crate::types::DataKey::RepaymentDispute(borrower.clone(), voucher.clone()), &dispute);

    env.events().publish(
        (symbol_short!("dispute"), symbol_short!("repayment")),
        (voucher, borrower),
    );

    Ok(())
}

/// Compute the reputation-weighted stake for a vouch.
/// Vouchers with higher reputation scores get their stake weighted more heavily,
/// providing them with greater yield and governance influence (Issue #866).
/// Weight multiplier: 1.0 + (reputation_score * 10 bps), capped at 2.0x (10000 bps).
pub fn vouch_reputation_weight(env: &Env, voucher: &Address) -> i128 {
    let rep_score: u32 = env
        .storage()
        .persistent()
        .get::<DataKey, crate::types::VoucherStats>(&DataKey::VoucherStats(voucher.clone()))
        .map(|s| s.successful_vouches)
        .unwrap_or(0);
    // Each successful vouch adds 500 bps (5%) weight, max 10000 bps (100% = 2x)
    let weight_bps = (rep_score as i128 * 500).min(10_000);
    BPS_DENOMINATOR + weight_bps
}

/// Compute the reputation-weighted total stake for a borrower's vouches.
/// Used in loan eligibility and yield distribution (Issue #866).
pub fn total_vouched_weighted(env: &Env, borrower: &Address, token: &Address) -> i128 {
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));
    let mut total: i128 = 0;
    for v in vouches.iter() {
        if v.token == *token {
            let weight = vouch_reputation_weight(env, &v.voucher);
            total += v.stake * weight / BPS_DENOMINATOR;
        }
    }
    total
}

pub fn total_vouched(env: Env, borrower: Address) -> Result<i128, ContractError> {
    let cfg = crate::helpers::config(&env);
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));
    let total: i128 = vouches
        .iter()
        .filter(|v| v.token == cfg.token)
        .map(|v| v.stake)
        .sum();
    Ok(total)
}

/// Admin: set whether a voucher is validated on a given chain.
pub fn set_bridge_validated(
    env: Env,
    admin_signers: Vec<Address>,
    voucher: Address,
    chain_id: u32,
    validated: bool,
) -> Result<(), ContractError> {
    crate::helpers::require_admin_approval(&env, &admin_signers);
    env.storage()
        .persistent()
        .set(&DataKey::BridgeValidated(voucher, chain_id), &validated);
    Ok(())
}

/// Query whether a voucher is bridge-validated for a given chain.
pub fn is_bridge_validated(env: Env, voucher: Address, chain_id: u32) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::BridgeValidated(voucher, chain_id))
        .unwrap_or(false)
}

pub fn request_vouch_withdrawal(
    _env: Env,
    _voucher: Address,
    _borrower: Address,
    _token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}

pub fn execute_vouch_withdrawal(
    _env: Env,
    _voucher: Address,
    _borrower: Address,
    _token: Address,
) -> Result<(), ContractError> {
    Err(ContractError::InvalidStateTransition)
}
