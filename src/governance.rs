use crate::errors::ContractError;
use crate::helpers::{add_slash_balance, config, get_active_loan_record, require_not_paused};
use crate::types::{DataKey, SlashVoteRecord, VouchRecord};
use soroban_sdk::{symbol_short, Address, Env, Vec};

/// Default quorum: 50% of total vouched stake must approve.
const DEFAULT_SLASH_VOTE_QUORUM_BPS: u32 = 5_000;

/// Cast a governance vote on whether `borrower` should be slashed.
///
/// - Only active vouchers (those with a stake in `Vouches(borrower)`) may vote.
/// - Votes are weighted by the voucher's current stake.
/// - When `approve_stake * 10_000 / total_stake >= quorum_bps`, slash is auto-executed.
pub fn vote_slash(
    env: Env,
    voucher: Address,
    borrower: Address,
    approve: bool,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;

    // Check for an already-executed vote first (before the active-loan guard).
    let existing_vote: Option<SlashVoteRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::SlashVote(borrower.clone()));
    if let Some(ref v) = existing_vote {
        if v.executed {
            return Err(ContractError::SlashAlreadyExecuted);
        }
    }

    // Borrower must have an active loan to be slashable.
    let loan = get_active_loan_record(&env, &borrower)?;
    if loan.repaid || loan.defaulted {
        return Err(ContractError::NoActiveLoan);
    }

    // Fetch vouches and find this voucher's stake.
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    let voucher_stake = vouches
        .iter()
        .find(|v| v.voucher == voucher)
        .map(|v| v.stake)
        .ok_or(ContractError::VoucherNotFound)?;

    let total_stake: i128 = vouches.iter().map(|v| v.stake).sum();

    // Load or initialise the vote record.
    let mut vote: SlashVoteRecord = env
        .storage()
        .persistent()
        .get(&DataKey::SlashVote(borrower.clone()))
        .unwrap_or(SlashVoteRecord {
            approve_stake: 0,
            reject_stake: 0,
            voters: Vec::new(&env),
            executed: false,
        });

    if vote.executed {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    // Prevent double-voting.
    if vote.voters.iter().any(|v| v == voucher) {
        return Err(ContractError::AlreadyVoted);
    }

    if approve {
        vote.approve_stake += voucher_stake;
    } else {
        vote.reject_stake += voucher_stake;
    }
    vote.voters.push_back(voucher.clone());

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("voted")),
        (voucher.clone(), borrower.clone(), approve, voucher_stake),
    );

    // Check quorum.
    let quorum_bps: u32 = env
        .storage()
        .instance()
        .get(&DataKey::SlashVoteQuorum)
        .unwrap_or(DEFAULT_SLASH_VOTE_QUORUM_BPS);

    let quorum_reached = total_stake > 0
        && vote.approve_stake * 10_000 / total_stake >= quorum_bps as i128;

    if quorum_reached {
        vote.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::SlashVote(borrower.clone()), &vote);
        execute_slash(&env, &borrower)?;
    } else {
        env.storage()
            .persistent()
            .set(&DataKey::SlashVote(borrower.clone()), &vote);
    }

    Ok(())
}

/// Returns the current slash vote record for a borrower, if any.
pub fn get_slash_vote(env: Env, borrower: Address) -> Option<SlashVoteRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::SlashVote(borrower))
}

/// Set the quorum threshold (in basis points) required to auto-execute a slash.
/// Requires admin approval — called from admin module.
pub fn set_slash_vote_quorum(env: &Env, quorum_bps: u32) {
    assert!(
        quorum_bps > 0 && quorum_bps <= 10_000,
        "quorum_bps must be 1-10000"
    );
    env.storage()
        .instance()
        .set(&DataKey::SlashVoteQuorum, &quorum_bps);
}

pub fn get_slash_vote_quorum(env: Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::SlashVoteQuorum)
        .unwrap_or(DEFAULT_SLASH_VOTE_QUORUM_BPS)
}

// ── Internal ──────────────────────────────────────────────────────────────────

fn execute_slash(env: &Env, borrower: &Address) -> Result<(), ContractError> {
    let cfg = config(env);

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    // Mark loan as defaulted first so we can read token_address.
    let mut loan = get_active_loan_record(env, borrower)?;
    let loan_token = soroban_sdk::token::Client::new(env, &loan.token_address);

    let mut total_slashed: i128 = 0;

    for v in vouches.iter() {
        if v.token != loan.token_address {
            continue;
        }
        let slash_amount = v.stake * cfg.slash_bps / 10_000;
        let remaining = v.stake - slash_amount;
        total_slashed += slash_amount;

        if remaining > 0 {
            loan_token.transfer(&env.current_contract_address(), &v.voucher, &remaining);
        }
    }

    add_slash_balance(env, total_slashed);

    loan.defaulted = true;
    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);
    env.storage()
        .persistent()
        .remove(&DataKey::ActiveLoan(borrower.clone()));

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::DefaultCount(borrower.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::DefaultCount(borrower.clone()), &(count + 1));

    env.storage()
        .persistent()
        .remove(&DataKey::Vouches(borrower.clone()));

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("slashed")),
        (borrower.clone(), total_slashed),
    );

    Ok(())
}
