/// #634: Vouch Liquidity Mining
use soroban_sdk::{symbol_short, token, Address, Env, Vec};

use crate::{
    errors::ContractError,
    helpers::{config, require_not_paused},
    types::{DataKey, VouchRecord, LIQUIDITY_MINING_EPOCH_SECS},
};

/// Claim accumulated liquidity mining rewards for a voucher.
/// Rewards = total_stake * liquidity_mining_rate_bps / 10_000 * full_epochs_elapsed.
pub fn claim_liquidity_mining_reward(env: Env, voucher: Address) -> Result<i128, ContractError> {
    require_not_paused(&env)?;
    voucher.require_auth();

    let cfg = config(&env);
    let now = env.ledger().timestamp();

    let last_claim: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::LastMiningClaim(voucher.clone()))
        .unwrap_or(now.saturating_sub(LIQUIDITY_MINING_EPOCH_SECS));

    let full_epochs = now.saturating_sub(last_claim) / LIQUIDITY_MINING_EPOCH_SECS;
    if full_epochs == 0 {
        return Ok(0);
    }

    let total_stake = voucher_total_stake(&env, &voucher);
    if total_stake <= 0 {
        return Ok(0);
    }

    let reward = total_stake * cfg.liquidity_mining_rate_bps as i128 / 10_000 * full_epochs as i128;
    if reward <= 0 {
        return Ok(0);
    }

    env.storage().persistent().set(
        &DataKey::LastMiningClaim(voucher.clone()),
        &(last_claim + full_epochs * LIQUIDITY_MINING_EPOCH_SECS),
    );

    token::Client::new(&env, &cfg.token)
        .transfer(&env.current_contract_address(), &voucher, &reward);

    env.events()
        .publish((symbol_short!("lm_claim"),), (voucher, reward, full_epochs));

    Ok(reward)
}

/// Return pending (unclaimed) reward without mutating state.
pub fn get_pending_mining_reward(env: Env, voucher: Address) -> i128 {
    let cfg = config(&env);
    let now = env.ledger().timestamp();
    let last_claim: u64 = env
        .storage()
        .persistent()
        .get(&DataKey::LastMiningClaim(voucher.clone()))
        .unwrap_or(now.saturating_sub(LIQUIDITY_MINING_EPOCH_SECS));

    let full_epochs = now.saturating_sub(last_claim) / LIQUIDITY_MINING_EPOCH_SECS;
    if full_epochs == 0 {
        return 0;
    }
    let total_stake = voucher_total_stake(&env, &voucher);
    total_stake * cfg.liquidity_mining_rate_bps as i128 / 10_000 * full_epochs as i128
}

fn voucher_total_stake(env: &Env, voucher: &Address) -> i128 {
    let history: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut total: i128 = 0;
    for borrower in history.iter() {
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or_else(|| Vec::new(env));
        for v in vouches.iter() {
            if v.voucher == *voucher {
                total = total.saturating_add(v.amount);
            }
        }
    }
    total
}
