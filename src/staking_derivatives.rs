/// #636: Vouch Staking Derivatives
use soroban_sdk::{symbol_short, Address, Env, Vec};

use crate::{
    errors::ContractError,
    helpers::require_not_paused,
    types::{DataKey, StakingDerivativeRecord, VouchRecord},
};

pub fn mint_staking_derivative(env: Env, voucher: Address, borrower: Address) -> Result<(), ContractError> {
    require_not_paused(&env)?;
    voucher.require_auth();

    let stake = get_vouch_stake(&env, &voucher, &borrower)?;

    if env.storage().persistent().has(&DataKey::StakingDerivative(voucher.clone(), borrower.clone())) {
        return Err(ContractError::DuplicateVouch);
    }

    env.storage().persistent().set(
        &DataKey::StakingDerivative(voucher.clone(), borrower.clone()),
        &StakingDerivativeRecord {
            voucher: voucher.clone(),
            borrower: borrower.clone(),
            stake_amount: stake,
            minted_at: env.ledger().timestamp(),
            current_holder: voucher.clone(),
            is_active: true,
        },
    );

    env.events().publish((symbol_short!("drv"), symbol_short!("mint")), (voucher, borrower, stake));
    Ok(())
}

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
    let mut d: StakingDerivativeRecord = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContractError::VoucherNotFound)?;

    if !d.is_active {
        return Err(ContractError::InvalidStateTransition);
    }
    if d.current_holder != from {
        return Err(ContractError::UnauthorizedCaller);
    }

    d.current_holder = to.clone();
    env.storage().persistent().set(&key, &d);

    env.events().publish((symbol_short!("drv"), symbol_short!("xfer")), (from, to, original_voucher, borrower));
    Ok(())
}

pub fn get_staking_derivative(env: Env, voucher: Address, borrower: Address) -> Option<StakingDerivativeRecord> {
    env.storage().persistent().get(&DataKey::StakingDerivative(voucher, borrower))
}

pub fn burn_staking_derivative(env: &Env, voucher: &Address, borrower: &Address) {
    let key = DataKey::StakingDerivative(voucher.clone(), borrower.clone());
    if let Some(mut d) = env.storage().persistent().get::<DataKey, StakingDerivativeRecord>(&key) {
        d.is_active = false;
        env.storage().persistent().set(&key, &d);
    }
}

fn get_vouch_stake(env: &Env, voucher: &Address, borrower: &Address) -> Result<i128, ContractError> {
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .ok_or(ContractError::NoVouchesForBorrower)?;
    for v in vouches.iter() {
        if v.voucher == *voucher {
            return Ok(v.stake);
        }
    }
    Err(ContractError::VoucherNotFound)
}
