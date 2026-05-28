use crate::errors::ContractError;
use crate::helpers::{
    add_slash_balance, config, get_active_loan_record, get_latest_loan_record, has_active_loan,
    require_admin_approval, require_governance_participant, require_not_paused,
};
use crate::types::{
    DataKey, LoanStatus, PendingSlashRecord, SlashAppealRecord, SlashThresholdProposal,
    SlashVoteRecord, TimelockAction, TimelockProposal, VouchRecord, BPS_DENOMINATOR,
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

    let cfg = config(&env);
    let now = env.ledger().timestamp();
    if cfg.slash_cooldown_seconds > 0 {
        if let Some(last) = env
            .storage()
            .persistent()
            .get(&DataKey::LastSlashedAt(borrower.clone()))
        {
            if now.saturating_sub(last) < cfg.slash_cooldown_seconds {
                return Err(ContractError::SlashCooldownActive);
            }
        }
    }

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
        if has_active_loan(&env, &borrower) {
            vote = SlashVoteRecord {
                approve_stake: 0,
                reject_stake: 0,
                voters: Vec::new(&env),
                executed: false,
            };
        } else {
            panic_with_error!(&env, ContractError::SlashAlreadyExecuted);
        }
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
        
        // Instead of immediately executing, create a pending slash record
        let cfg = config(&env);
        let now = env.ledger().timestamp();
        let executable_at = now + cfg.slash_delay_seconds;
        
        let pending_slash = PendingSlashRecord {
            borrower: borrower.clone(),
            approved_at: now,
            executable_at,
            executed: false,
        };
        
        env.storage()
            .persistent()
            .set(&DataKey::PendingSlashExecution(borrower.clone()), &pending_slash);
        
        env.events().publish(
            (symbol_short!("gov"), symbol_short!("slash_pending")),
            (borrower.clone(), now, executable_at),
        );
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

/// Execute a pending slash after the delay period has passed.
/// Anyone can call this function to execute a pending slash once the delay has elapsed.
pub fn execute_pending_slash(env: Env, borrower: Address) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    let pending_slash: PendingSlashRecord = env
        .storage()
        .persistent()
        .get(&DataKey::PendingSlashExecution(borrower.clone()))
        .ok_or(ContractError::SlashVoteNotFound)?;

    if pending_slash.executed {
        return Err(ContractError::SlashAlreadyExecuted);
    }

    // Check if delay period has passed
    let now = env.ledger().timestamp();
    if now < pending_slash.executable_at {
        return Err(ContractError::DelayNotElapsed);
    }

    // Mark as executed and execute the slash
    let mut updated_pending = pending_slash;
    updated_pending.executed = true;
    env.storage()
        .persistent()
        .set(&DataKey::PendingSlashExecution(borrower.clone()), &updated_pending);

    execute_slash(&env, &borrower)?;

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("slash_executed")),
        (borrower.clone(), now),
    );

    Ok(())
}

// ── Internal ──────────────────────────────────────────────────────────────────

fn next_slash_id(env: &Env) -> u64 {
    let id = env
        .storage()
        .instance()
        .get(&DataKey::SlashRecordCounter)
        .unwrap_or(0u64)
        .checked_add(1)
        .expect("slash ID overflow");
    env.storage()
        .instance()
        .set(&DataKey::SlashRecordCounter, &id);
    id
}

pub fn get_slash_record(env: Env, slash_id: u64) -> Option<SlashRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::SlashRecord(slash_id))
}

pub fn get_slash_record_for_borrower(env: Env, borrower: Address) -> Option<SlashRecord> {
    env.storage()
        .persistent()
        .get(&DataKey::SlashAudit(borrower))
}

pub fn reverse_slash(
    env: Env,
    admin_signers: Vec<Address>,
    slash_id: u64,
    reason: soroban_sdk::String,
) -> Result<(), ContractError> {
    require_not_paused(&env)?;
    require_admin_approval(&env, &admin_signers);

    let mut record: SlashRecord = env
        .storage()
        .persistent()
        .get(&DataKey::SlashRecord(slash_id))
        .ok_or(ContractError::SlashRecordNotFound)?;

    if record.reversed {
        return Err(ContractError::SlashAlreadyReversed);
    }

    let loan = get_latest_loan_record(&env, &record.borrower).ok_or(ContractError::NoActiveLoan)?;
    let token = soroban_sdk::token::Client::new(&env, &loan.token_address);
    let contract = env.current_contract_address();

    token.transfer(&contract, &record.borrower, &record.total_slashed);

    record.reversed = true;
    record.reversal_reason = Some(reason.clone());

    env.storage()
        .persistent()
        .set(&DataKey::SlashRecord(slash_id), &record);
    env.storage()
        .persistent()
        .set(&DataKey::SlashAudit(record.borrower.clone()), &record);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("rev_slsh")),
        (slash_id, record.borrower.clone(), reason),
    );

    Ok(())
}

fn execute_slash(env: &Env, borrower: &Address) -> Result<(), ContractError> {
    let cfg = config(env);
    let now = env.ledger().timestamp();

    if cfg.slash_cooldown_seconds > 0 {
        if let Some(last) = env
            .storage()
            .persistent()
            .get(&DataKey::LastSlashedAt(borrower.clone()))
        {
            if now.saturating_sub(last) < cfg.slash_cooldown_seconds {
                return Err(ContractError::SlashCooldownActive);
            }
        }
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(env));

    let mut loan = get_active_loan_record(env, borrower)?;
    if loan.status == LoanStatus::Defaulted {
        panic_with_error!(env, ContractError::SlashAlreadyExecuted);
    }
    let loan_token = soroban_sdk::token::Client::new(env, &loan.token_address);

    let mut total_slashed: i128 = 0;
    let mut remaining_vouches: Vec<VouchRecord> = Vec::new(env);

    for v in vouches.iter() {
        if v.token != loan.token_address {
            remaining_vouches.push_back(v);
            continue;
        }
        let slash_amount = v.stake * cfg.slash_bps / BPS_DENOMINATOR;
        let remaining = v.stake - slash_amount;
        total_slashed += slash_amount;

        if remaining > 0 {
            loan_token.transfer(&env.current_contract_address(), &v.voucher, &remaining);
        }

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

    env.storage()
        .persistent()
        .set(&DataKey::LastSlashedAt(borrower.clone()), &now);

    // Store slashed funds in escrow instead of immediately burning
    let release_timestamp = now + crate::types::SLASH_ESCROW_PERIOD;
    env.storage()
        .persistent()
        .set(&DataKey::SlashEscrow(borrower.clone()), &(total_slashed, release_timestamp));

    loan.status = LoanStatus::Defaulted;
    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);
    env.storage()
        .persistent()
        .set(&DataKey::LatestLoan(borrower.clone()), &loan.id);
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

    if remaining_vouches.is_empty() {
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));
    } else {
        env.storage()
            .persistent()
        .set(&DataKey::Vouches(borrower.clone()), &remaining_vouches);
    }

    let slash_id = next_slash_id(env);
    let record = SlashRecord {
        slash_id,
        borrower: borrower.clone(),
        loan_id: loan.id,
        loan_amount: loan.amount,
        total_slashed,
        slash_timestamp: now,
        recovery_amount: 0,
        reversal_reason: None,
        reversed: false,
    };
    env.storage()
        .persistent()
        .set(&DataKey::SlashRecord(slash_id), &record);
    env.storage()
        .persistent()
        .set(&DataKey::SlashAudit(borrower.clone()), &record);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("slashed")),
        (borrower.clone(), total_slashed, slash_id),
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

// ── Issue #680: Slash threshold governance voting ─────────────────────────────

pub fn propose_slash_threshold(
    env: Env,
    proposer: Address,
    new_threshold: i128,
) -> Result<u64, ContractError> {
    proposer.require_auth();
    require_not_paused(&env)?;
    require_governance_participant(&env, &proposer)?;

    if new_threshold <= 0 || new_threshold > 10_000 {
        return Err(ContractError::InvalidBps);
    }

    let proposal_id: u64 = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::SlashThresholdProposalCounter)
        .unwrap_or(0u64)
        .checked_add(1)
        .expect("proposal id overflow");

    let proposal = SlashThresholdProposal {
        id: proposal_id,
        proposer: proposer.clone(),
        proposed_threshold: new_threshold,
        proposed_at: env.ledger().timestamp(),
        approve_votes: 0,
        reject_votes: 0,
        voters: Vec::new(&env),
        finalized: false,
    };

    env.storage()
        .instance()
        .set(&DataKey::SlashThresholdProposal(proposal_id), &proposal);
    env.storage()
        .instance()
        .set(&DataKey::SlashThresholdProposalCounter, &proposal_id);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("sth_prop")),
        (proposal_id, proposer, new_threshold),
    );

    Ok(proposal_id)
}

pub fn vote_slash_threshold(
    env: Env,
    voter: Address,
    proposal_id: u64,
    approve: bool,
) -> Result<(), ContractError> {
    voter.require_auth();
    require_not_paused(&env)?;
    require_governance_participant(&env, &voter)?;

    let mut proposal: SlashThresholdProposal = env
        .storage()
        .instance()
        .get(&DataKey::SlashThresholdProposal(proposal_id))
        .ok_or(ContractError::ProposalNotFound)?;

    if proposal.finalized {
        return Err(ContractError::ProposalAlreadyFinalized);
    }

    let cfg = config(&env);
    let now = env.ledger().timestamp();
    if now >= proposal.proposed_at + cfg.voting_period_seconds {
        return Err(ContractError::VotingPeriodEnded);
    }

    if proposal.voters.iter().any(|v| v == voter) {
        return Err(ContractError::AlreadyVoted);
    }

    if approve {
        proposal.approve_votes += 1;
    } else {
        proposal.reject_votes += 1;
    }
    proposal.voters.push_back(voter.clone());

    env.storage()
        .instance()
        .set(&DataKey::SlashThresholdProposal(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("gov"), symbol_short!("sth_vote")),
        (proposal_id, voter, approve),
    );

    Ok(())
}

pub fn finalize_slash_threshold(env: Env, proposal_id: u64) -> Result<(), ContractError> {
    require_not_paused(&env)?;

    let mut proposal: SlashThresholdProposal = env
        .storage()
        .instance()
        .get(&DataKey::SlashThresholdProposal(proposal_id))
        .ok_or(ContractError::ProposalNotFound)?;

    if proposal.finalized {
        return Err(ContractError::ProposalAlreadyFinalized);
    }

    let cfg = config(&env);
    let now = env.ledger().timestamp();
    let voting_end = proposal.proposed_at + cfg.voting_period_seconds;

    if now < voting_end {
        return Err(ContractError::TimelockNotReady);
    }
    if now > voting_end + cfg.voting_period_seconds {
        return Err(ContractError::TimelockExpired);
    }

    proposal.finalized = true;
    env.storage()
        .instance()
        .set(&DataKey::SlashThresholdProposal(proposal_id), &proposal);

    if proposal.approve_votes > proposal.reject_votes {
        let mut cfg = config(&env);
        cfg.slash_bps = proposal.proposed_threshold;
        env.storage().instance().set(&DataKey::Config, &cfg);
        env.events().publish(
            (symbol_short!("gov"), symbol_short!("sth_ok")),
            (proposal_id, proposal.proposed_threshold),
        );
    } else {
        env.events().publish(
            (symbol_short!("gov"), symbol_short!("sth_no")),
            proposal_id,
        );
    }

    Ok(())
}

pub fn get_slash_threshold_proposal(
    env: Env,
    proposal_id: u64,
) -> Option<SlashThresholdProposal> {
    env.storage()
        .instance()
        .get(&DataKey::SlashThresholdProposal(proposal_id))
}
