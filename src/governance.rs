use crate::errors::ContractError;
use crate::helpers::{
    add_slash_balance, config, get_active_loan_record, get_latest_loan_record, require_not_paused,
};
use crate::types::{
    DataKey, LoanStatus, SlashVoteRecord, TimelockAction, TimelockProposal, VouchRecord,
    BPS_DENOMINATOR, SlashAppealRecord,
};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, Vec};

/// Default quorum: 50% of total vouched stake must approve.
const DEFAULT_SLASH_VOTE_QUORUM_BPS: u32 = 5_000;

/// Cast a governance vote on whether `borrower` should be slashed.
///
/// - Only active vouchers (those with a stake in `Vouches(borrower)`) may vote.
/// - Votes are weighted by the voucher's current stake.
/// - When `approve_stake * BPS_DENOMINATOR / total_stake >= quorum_bps`, slash is auto-executed.
pub fn vote_slash(
    env: Env,
    voucher: Address,
    borrower: Address,
    approve: bool,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;

    // If the borrower's latest loan is already repaid, panic with a clear message.
    if let Some(latest) = get_latest_loan_record(&env, &borrower) {
        assert!(
            latest.status != LoanStatus::Repaid,
            "loan already repaid"
        );
    }

    // Borrower must have an active loan to be slashable.
    let loan = get_active_loan_record(&env, &borrower)?;
    if loan.status != crate::types::LoanStatus::Active {
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
        panic_with_error!(&env, ContractError::SlashAlreadyExecuted);
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

    // Use ceiling division to prevent rounding down: (approve_stake * BPS_DENOMINATOR + total_stake - 1) / total_stake
    let quorum_reached = total_stake > 0
        && (vote.approve_stake * BPS_DENOMINATOR + total_stake - 1) / total_stake
            >= quorum_bps as i128;

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
    if quorum_bps > 10_000 {
        panic_with_error!(env, ContractError::InvalidBps);
    }
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

/// Execute a slash vote if quorum has been met.
/// Anyone can call this function to execute a slash once quorum is reached.
pub fn execute_slash_vote(env: Env, borrower: Address) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    let vote: crate::types::SlashVoteRecord = env
        .storage()
        .persistent()
        .get(&DataKey::SlashVote(borrower.clone()))
        .ok_or(ContractError::SlashVoteNotFound)?;

    if vote.executed {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    // Get total stake for the borrower
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));
    let total_stake: i128 = vouches.iter().map(|v| v.stake).sum();

    // Retrieve quorum threshold
    let quorum_bps: u32 = get_slash_vote_quorum(env.clone());

    // Calculate required quorum stake
    let quorum_stake = total_stake * quorum_bps as i128 / 10_000;

    // Check if approval stake meets quorum
    if vote.approve_stake < quorum_stake {
        return Err(ContractError::QuorumNotMet);
    }

    // Mark as executed and execute the slash
    let mut updated_vote = vote;
    updated_vote.executed = true;
    env.storage()
        .persistent()
        .set(&DataKey::SlashVote(borrower.clone()), &updated_vote);

    execute_slash(&env, &borrower)?;

    Ok(())
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
    if loan.status == crate::types::LoanStatus::Defaulted {
        panic_with_error!(env, ContractError::SlashAlreadyExecuted);
    }
    let loan_token = soroban_sdk::token::Client::new(env, &loan.token_address);

    // Calculate dynamic slash threshold based on protocol health
    let effective_slash_bps = crate::helpers::calculate_dynamic_slash_threshold(env);

    // Issue #551: Calculate total stake for proportional slashing
    let mut total_loan_token_stake: i128 = 0;
    for v in vouches.iter() {
        if v.token == loan.token_address {
            total_loan_token_stake += v.stake;
        }
    }

    let mut total_slashed: i128 = 0;
    let mut remaining_vouches: Vec<VouchRecord> = Vec::new(env);

    for v in vouches.iter() {
        if v.token != loan.token_address {
            // Keep non-loan-token vouches
            remaining_vouches.push_back(v);
            continue;
        }
        
        // Issue #551: Proportional slashing based on voucher's share of total stake
        let voucher_share_bps = if total_loan_token_stake > 0 {
            (v.stake * BPS_DENOMINATOR) / total_loan_token_stake
        } else {
            0
        };
        
        // Slash amount uses dynamic threshold instead of static cfg.slash_bps
        let slash_amount = loan.amount * voucher_share_bps / BPS_DENOMINATOR * effective_slash_bps / BPS_DENOMINATOR;
        let remaining = v.stake - slash_amount;
        total_slashed += slash_amount;

        if remaining > 0 {
            loan_token.transfer(&env.current_contract_address(), &v.voucher, &remaining);
        }

        // Issue #602: Update voucher reputation stats on slash.
        let mut stats: crate::types::VoucherStats = env
            .storage()
            .persistent()
            .get(&DataKey::VoucherStats(v.voucher.clone()))
            .unwrap_or(crate::types::VoucherStats {
                successful_vouches: 0,
                total_vouches_slashed: 0,
                total_yield_earned: 0,
                total_slashed: 0,
            });
        stats.total_vouches_slashed += 1;
        stats.total_slashed += slash_amount;
        env.storage()
            .persistent()
            .set(&DataKey::VoucherStats(v.voucher.clone()), &stats);
    }

    // Store slashed funds in escrow instead of immediately burning
    let now = env.ledger().timestamp();
    let release_timestamp = now + crate::types::SLASH_ESCROW_PERIOD;
    env.storage()
        .persistent()
        .set(&DataKey::SlashEscrow(borrower.clone()), &(total_slashed, release_timestamp));

    loan.status = crate::types::LoanStatus::Defaulted;
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

    // Only remove vouches if all were processed; otherwise keep remaining vouches
    if remaining_vouches.is_empty() {
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));
    } else {
        env.storage()
            .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &remaining_vouches);
    }

    // Emit event with both static and dynamic slash rates for transparency
    env.events().publish(
        (symbol_short!("gov"), symbol_short!("slashed")),
        (borrower.clone(), total_slashed, cfg.slash_bps, effective_slash_bps),
    );

    // Log slash audit record (Issue #536)
    env.storage().persistent().set(
        &DataKey::SlashAudit(borrower.clone()),
        &crate::types::SlashAuditRecord {
            borrower: borrower.clone(),
            loan_amount: loan.amount,
            total_slashed,
            slash_timestamp: env.ledger().timestamp(),
        },
    );

    Ok(())
}

/// ── Issue 109: Slash Proposal Confirmation Window ──
///
/// Implements a two-step slash with timelock pattern:
/// 1. propose_slash: Admin creates a proposal, sets execution time (eta)
/// 2. execute_slash_proposal: After delay, anyone can execute

/// Propose a slash action with a delay before execution.
/// This implements the "confirmation window" for the slash action.
pub fn propose_slash(
    env: Env,
    proposer: Address,
    borrower: Address,
    delay_secs: u64,
) -> Result<u64, ContractError> {
    proposer.require_auth();
    require_not_paused(&env)?;

    // Verify borrower has an active loan
    let _loan = get_active_loan_record(&env, &borrower)?;

    // Get or initialize timelock counter
    let proposal_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::TimelockCounter)
        .unwrap_or(0u64)
        .checked_add(1)
        .expect("proposal ID overflow");

    let eta = env.ledger().timestamp() + delay_secs;

    let proposal = TimelockProposal {
        id: proposal_id,
        action: TimelockAction::Slash(borrower.clone()),
        proposer: proposer.clone(),
        eta,
        executed: false,
        cancelled: false,
    };

    env.storage()
        .instance()
        .set(&DataKey::Timelock(proposal_id), &proposal);
    env.storage()
        .instance()
        .set(&DataKey::TimelockCounter, &proposal_id);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("proposed")),
        (proposal_id, proposer, borrower, eta),
    );

    Ok(proposal_id)
}

/// Execute a previously proposed slash action after the delay has passed.
pub fn execute_slash_proposal(env: Env, proposal_id: u64) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    // Get the proposal
    let mut proposal: TimelockProposal = env
        .storage()
        .instance()
        .get(&DataKey::Timelock(proposal_id))
        .ok_or(ContractError::NoActiveLoan)?; // Use existing error as placeholder

    // Check proposal state
    if proposal.executed {
        return Err(ContractError::SlashAlreadyExecuted);
    }
    if proposal.cancelled {
        return Err(ContractError::NoActiveLoan); // Use existing error as placeholder
    }

    // Check delay has passed
    if env.ledger().timestamp() < proposal.eta {
        return Err(ContractError::NoActiveLoan); // Use existing error as placeholder
    }

    // Check expiry (72 hours from eta)
    const TIMELOCK_EXPIRY: u64 = 72 * 60 * 60;
    if env.ledger().timestamp() > proposal.eta + TIMELOCK_EXPIRY {
        return Err(ContractError::NoActiveLoan); // Use existing error as placeholder
    }

    // Extract borrower from the Slash action
    if let TimelockAction::Slash(borrower) = &proposal.action {
        // Mark as executed before calling execute_slash to prevent reentrancy
        proposal.executed = true;
        env.storage()
            .instance()
            .set(&DataKey::Timelock(proposal_id), &proposal);

        // Execute the slash
        execute_slash(&env, borrower)?;

        env.events().publish(
            (symbol_short!("gov"), symbol_short!("executed")),
            (proposal_id, borrower.clone()),
        );

        Ok(())
    } else {
        Err(ContractError::NoActiveLoan) // Only Slash actions supported in this release
    }
}

/// Cancel a pending slash proposal (only by proposer or admin).
pub fn cancel_slash_proposal(
    env: Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), ContractError> {
    caller.require_auth();

    let mut proposal: TimelockProposal = env
        .storage()
        .instance()
        .get(&DataKey::Timelock(proposal_id))
        .ok_or(ContractError::NoActiveLoan)?;

    // Only proposer can cancel
    if caller != proposal.proposer {
        panic_with_error!(&env, ContractError::UnauthorizedCaller);
    }

    if proposal.executed || proposal.cancelled {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    proposal.cancelled = true;
    env.storage()
        .instance()
        .set(&DataKey::Timelock(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("cancelled")),
        (proposal_id, caller),
    );

    Ok(())
}

/// Get a timelock proposal by ID.
pub fn get_timelock_proposal(env: Env, proposal_id: u64) -> Option<TimelockProposal> {
    env.storage()
        .instance()
        .get(&DataKey::Timelock(proposal_id))
}

/// Issue #552: Appeal a slash decision. Only the slashed voucher can appeal.
pub fn appeal_slash(
    env: Env,
    voucher: Address,
    borrower: Address,
    evidence_hash: soroban_sdk::BytesN<32>,
) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;

    // Verify the loan was defaulted
    let loan = get_latest_loan_record(&env, &borrower)
        .ok_or(ContractError::NoActiveLoan)?;
    if loan.status != LoanStatus::Defaulted {
        return Err(ContractError::NoActiveLoan);
    }

    // Create appeal record
    let appeal = SlashAppealRecord {
        borrower: borrower.clone(),
        voucher: voucher.clone(),
        evidence_hash,
        appeal_timestamp: env.ledger().timestamp(),
        approved: None,
        admin_votes: Vec::new(&env),
    };

    env.storage()
        .persistent()
        .set(&DataKey::SlashAppeal(borrower.clone(), voucher.clone()), &appeal);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("appeal")),
        (voucher, borrower),
    );

    Ok(())
}

/// Issue #552: Admin votes on a slash appeal.
pub fn vote_on_slash_appeal(
    env: Env,
    admin_signers: Vec<Address>,
    borrower: Address,
    voucher: Address,
    approve: bool,
) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    // Verify admin approval
    crate::helpers::require_admin_approval(&env, &admin_signers);

    let mut appeal: SlashAppealRecord = env
        .storage()
        .persistent()
        .get(&DataKey::SlashAppeal(borrower.clone(), voucher.clone()))
        .ok_or(ContractError::NoActiveLoan)?;

    if appeal.approved.is_some() {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    appeal.approved = Some(approve);
    appeal.admin_votes = admin_signers.clone();

    env.storage()
        .persistent()
        .set(&DataKey::SlashAppeal(borrower.clone(), voucher.clone()), &appeal);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("appl_vote")),
        (borrower, voucher, approve),
    );

    Ok(())
}

/// Issue #552: Execute a slash appeal if approved. Reverses the slash.
pub fn execute_slash_appeal(
    env: Env,
    borrower: Address,
    voucher: Address,
) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    let appeal: SlashAppealRecord = env
        .storage()
        .persistent()
        .get(&DataKey::SlashAppeal(borrower.clone(), voucher.clone()))
        .ok_or(ContractError::NoActiveLoan)?;

    if appeal.approved != Some(true) {
        return Err(ContractError::UnauthorizedCaller);
    }

    // Get the loan to find the token
    let loan = get_latest_loan_record(&env, &borrower)
        .ok_or(ContractError::NoActiveLoan)?;

    let token_client = soroban_sdk::token::Client::new(&env, &loan.token_address);

    // Restore the voucher's stake (50% of original, since 50% was slashed)
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    let original_stake = vouches
        .iter()
        .find(|v| v.voucher == voucher && v.token == loan.token_address)
        .map(|v| v.stake)
        .unwrap_or(0);

    // Restore 50% of the original stake (the slashed amount)
    let restored_amount = original_stake / 2;
    if restored_amount > 0 {
        token_client.transfer(
            &env.current_contract_address(),
            &voucher,
            &restored_amount,
        );
    }

    // Remove the appeal record
    env.storage()
        .persistent()
        .remove(&DataKey::SlashAppeal(borrower.clone(), voucher.clone()));

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("appl_exec")),
        (borrower, voucher),
    );

    Ok(())
}
