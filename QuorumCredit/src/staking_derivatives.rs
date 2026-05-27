/// #636: Vouch Staking Derivatives
///
/// Vouchers can mint a derivative record representing their stake in a borrower.
/// The derivative can be transferred to another address, enabling secondary market trading.
/// Redeeming a derivative returns the underlying stake claim to the new holder.
use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::ContractError,
    helpers::require_not_paused,
    types::{DataKey, StakingDerivativeRecord, VouchRecord},
};

/// Mint a staking derivative for an existing vouch.
/// The voucher must have an active vouch for the borrower.
pub fn mint_staking_derivative(
    env: Env,
    voucher: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    require_not_paused(&env)?;
    voucher.require_auth();

    // Verify the vouch exists
    let stake = get_vouch_stake(&env, &voucher, &borrower)?;

    // Ensure no derivative already exists
    if env
        .storage()
        .persistent()
        .has(&DataKey::StakingDerivative(voucher.clone(), borrower.clone()))
    {
        return Err(ContractError::DuplicateVouch); // reuse: derivative already minted
    }

    let derivative = StakingDerivativeRecord {
        voucher: voucher.clone(),
        borrower: borrower.clone(),
        stake_amount: stake,
        minted_at: env.ledger().timestamp(),
        current_holder: voucher.clone(),
        is_active: true,
    };

    env.storage().persistent().set(
        &DataKey::StakingDerivative(voucher.clone(), borrower.clone()),
        &derivative,
    );

    env.events()
        .publish((symbol_short!("deriv_mnt"),), (voucher, borrower, stake));

    Ok(())
}

/// Transfer a staking derivative to a new holder (secondary market trade).
pub fn transfer_staking_derivative(
    env: Env,
    from: Address,
    to: Address,
    original_voucher: Address,
    borrower: Address,
) -> Result<(), ContractError> {
    require_not_paused(&env)?;
    from.require_auth();

    let key = DataKey::StakingDerivative(original_voucher.clone(), borrower.clone());
    let mut derivative: StakingDerivativeRecord = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::VoucherNotFound)?;

    if !derivative.is_active {
        return Err(ContractError::InvalidStateTransition);
    }
    if derivative.current_holder != from {
        return Err(ContractError::UnauthorizedCaller);
    }

    derivative.current_holder = to.clone();
    env.storage().persistent().set(&key, &derivative);

    env.events().publish(
        (symbol_short!("deriv_xfr"),),
        (from, to, original_voucher, borrower),
    );

    Ok(())
}

/// Get a staking derivative record.
pub fn get_staking_derivative(
    env: Env,
    voucher: Address,
    borrower: Address,
) -> Option<StakingDerivativeRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::StakingDerivative(voucher, borrower))
}

/// Burn/redeem a derivative (called when the underlying vouch is withdrawn).
pub fn burn_staking_derivative(env: &Env, voucher: &Address, borrower: &Address) {
    let key = DataKey::StakingDerivative(voucher.clone(), borrower.clone());
    if let Some(mut d) = env
        .storage()
        .persistent()
        .get::<DataKey, StakingDerivativeRecord>(&key)
    {
        d.is_active = false;
        env.storage().persistent().set(&key, &d);
    }
}

fn get_vouch_stake(env: &Env, voucher: &Address, borrower: &Address) -> Result<i128, ContractError> {
    let vouches: soroban_sdk::Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;

    for v in vouches.iter() {
        if v.voucher == *voucher {
            return Ok(v.amount);
        }
    }
    Err(ContractError::VoucherNotFound)
}
