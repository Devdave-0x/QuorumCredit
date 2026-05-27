/// #635: Vouch Snapshot for Governance
use soroban_sdk::{symbol_short, Address, Env, Vec};

use crate::{
    errors::ContractError,
    helpers::require_not_paused,
    types::{DataKey, VouchRecord, VouchSnapshotEntry, VouchSnapshotRecord},
};

pub fn take_vouch_snapshot(env: Env, caller: Address) -> Result<u32, ContractError> {
    require_not_paused(&env)?;
    caller.require_auth();

    let seq = env.ledger().sequence();
    let now = env.ledger().timestamp();

    let borrower_list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::BorrowerList)
        .unwrap_or_else(|| Vec::new(&env));

    let mut entries: Vec<VouchSnapshotEntry> = Vec::new(&env);
    for borrower in borrower_list.iter() {
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let total_stake: i128 = vouches.iter().map(|v| v.stake).sum();
        if total_stake > 0 {
            entries.push_back(VouchSnapshotEntry { borrower: borrower.clone(), total_stake });
        }
    }

    env.storage().persistent().set(
        &DataKey::VouchSnapshot(seq),
        &VouchSnapshotRecord { ledger_sequence: seq, timestamp: now, entries },
    );

    env.events().publish((symbol_short!("vouch"), symbol_short!("snap")), (seq, now));
    Ok(seq)
}

pub fn get_vouch_snapshot(env: Env, ledger_sequence: u32) -> Option<VouchSnapshotRecord> {
    env.storage().persistent().get(&DataKey::VouchSnapshot(ledger_sequence))
}

pub fn get_snapshot_stake(env: Env, ledger_sequence: u32, borrower: Address) -> i128 {
    let snapshot: VouchSnapshotRecord = match env
        .storage()
        .persistent()
        .get(&DataKey::VouchSnapshot(ledger_sequence))
    {
        Some(s) => s,
        None => return 0,
    };
    for entry in snapshot.entries.iter() {
        if entry.borrower == borrower {
            return entry.total_stake;
        }
    }
    0
}
