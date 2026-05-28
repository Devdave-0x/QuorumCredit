#![no_std]

mod admin;
mod errors;
mod governance;
mod helpers;
mod types;
mod vouch;
mod vouch_snapshot;

use soroban_sdk::{contract, contractimpl, symbol_short, token, Address, Env, String, Vec};

pub use errors::ContractError;
pub use types::*;

#[cfg(test)]
mod slash_threshold_voting_test;
#[cfg(test)]
mod slash_cooldown_test;
#[cfg(test)]
mod config_update_voting_test;
#[cfg(test)]
mod emergency_pause_test;
#[cfg(test)]
mod withdrawal_queue_test;
#[cfg(test)]
mod cross_chain_vouch_test;
#[cfg(test)]
mod property_stake_loan_invariants_test;

use crate::helpers::{
    config, get_active_loan_record, has_active_loan, loan_status as helper_loan_status,
    require_allowed_token, require_not_paused,
};

#[contract]
pub struct QuorumCreditContract;

#[contractimpl]
impl QuorumCreditContract {
    pub fn initialize(
        env: Env,
        deployer: Address,
        admins: Vec<Address>,
        admin_threshold: u32,
        token: Address,
    ) -> Result<(), ContractError> {
        deployer.require_auth();

        if env.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        helpers::validate_admin_config(&env, &admins, admin_threshold)?;
        helpers::require_valid_token(&env, &token)?;

        env.storage().instance().set(&DataKey::Deployer, &deployer);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                admins,
                admin_threshold,
                token,
                allowed_tokens: Vec::new(&env),
                yield_bps: DEFAULT_YIELD_BPS,
                slash_bps: DEFAULT_SLASH_BPS,
                max_vouchers: DEFAULT_MAX_VOUCHERS,
                min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
                loan_duration: DEFAULT_LOAN_DURATION,
                max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
                grace_period: 0,
                min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
                prepayment_penalty_bps: 0,
                liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
                voting_period_seconds: DEFAULT_VOTING_PERIOD_SECONDS,
                slash_cooldown_seconds: 0,
                emergency_pause_enabled: false,
                early_repayment_discount_bps: 0,
                oracle_address: None,
                slash_delay_seconds: 0,
            },
        );

        Ok(())
    }

    pub fn vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        chain_id: Option<u32>,
    ) -> Result<(), ContractError> {
        vouch::vouch(env, voucher, borrower, stake, token, chain_id)
    }

    /// Issue #632: Vouch with cross-chain support.
    /// chain_id=0 is native Stellar; non-zero requires prior bridge validation.
    pub fn vouch_cross_chain(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        chain_id: u32,
    ) -> Result<(), ContractError> {
        vouch::vouch_cross_chain(env, voucher, borrower, stake, token, chain_id)
    }

    /// Issue #632: Admin sets bridge validation status for a voucher on a given chain.
    pub fn set_bridge_validated(
        env: Env,
        admin_signers: Vec<Address>,
        voucher: Address,
        chain_id: u32,
        validated: bool,
    ) -> Result<(), ContractError> {
        vouch::set_bridge_validated(env, admin_signers, voucher, chain_id, validated)
    }

    /// Issue #632: Query bridge validation status.
    pub fn is_bridge_validated(env: Env, voucher: Address, chain_id: u32) -> bool {
        vouch::is_bridge_validated(env, voucher, chain_id)
    }

    pub fn batch_vouch(
        env: Env,
        voucher: Address,
        borrowers: Vec<Address>,
        stakes: Vec<i128>,
        token: Address,
        chain_id: Option<u32>,
    ) -> Result<(), ContractError> {
        vouch::batch_vouch(env, voucher, borrowers, stakes, token, chain_id)
    }

    pub fn increase_stake(
        env: Env,
        voucher: Address,
        borrower: Address,
        additional: i128,
    ) -> Result<(), ContractError> {
        vouch::increase_stake(env, voucher, borrower, additional)
    }

    pub fn decrease_stake(
        env: Env,
        voucher: Address,
        borrower: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        vouch::decrease_stake(env, voucher, borrower, amount)
    }

    pub fn withdraw_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::withdraw_vouch(env, voucher, borrower)
    }

    pub fn request_withdrawal(
        env: Env,
        voucher: Address,
        borrower: Address,
        priority_fee: i128,
    ) -> Result<(), ContractError> {
        vouch::request_withdrawal(env, voucher, borrower, priority_fee)
    }

    pub fn partial_withdraw(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::partial_withdraw(env, voucher, borrower)
    }

    pub fn get_withdrawal_queue(env: Env, borrower: Address) -> Vec<QueuedWithdrawal> {
        vouch::get_withdrawal_queue(env, borrower)
    }

    pub fn request_loan(
        env: Env,
        borrower: Address,
        amount: i128,
        threshold: i128,
        loan_purpose: String,
        token_addr: Address,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;

        if has_active_loan(&env, &borrower) {
            return Err(ContractError::ActiveLoanExists);
        }

        let token_client = require_allowed_token(&env, &token_addr)?;
        let cfg = config(&env);

        if amount < cfg.min_loan_amount {
            return Err(ContractError::LoanBelowMinAmount);
        }

        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let total_stake: i128 = vouches
            .iter()
            .filter(|v| v.token == token_addr)
            .map(|v| v.stake)
            .sum();

        if total_stake < threshold {
            return Err(ContractError::InsufficientFunds);
        }

        let now = env.ledger().timestamp();
        let loan_id = helpers::next_loan_id(&env);
        let total_yield = amount * cfg.yield_bps / 10_000;

        let loan = LoanRecord {
            id: loan_id,
            borrower: borrower.clone(),
            co_borrowers: Vec::new(&env),
            amount,
            amount_repaid: 0,
            total_yield,
            status: LoanStatus::Active,
            created_at: now,
            disbursement_timestamp: now,
            repayment_timestamp: None,
            deadline: now + cfg.loan_duration,
            loan_purpose,
            token_address: token_addr.clone(),
            amortization_schedule: Vec::new(&env),
            reminder_sent: false,
            risk_score: 0,
            deferment_periods: 0,
            maturity_date: None,
            rate_type: crate::types::RateType::Fixed,
            index_reference: None,
            escrow_status: EscrowStatus::None,
            retry_count: 0,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan_id), &loan);
        env.storage()
            .persistent()
            .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
        env.storage()
            .persistent()
            .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

        token_client.transfer(&env.current_contract_address(), &borrower, &amount);

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("created")),
            (borrower, amount),
        );

        Ok(())
    }

    pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;

        let mut loan = get_active_loan_record(&env, &borrower)?;

        if payment <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let cfg = config(&env);
        let now = env.ledger().timestamp();

        // #668: Apply early repayment discount if repaying before deadline
        let discount = if now < loan.deadline && cfg.early_repayment_discount_bps > 0 {
            loan.total_yield * cfg.early_repayment_discount_bps as i128 / 10_000
        } else {
            0
        };
        let effective_total_owed = loan.amount + loan.total_yield - discount;
        let outstanding = effective_total_owed - loan.amount_repaid;

        if payment > outstanding {
            return Err(ContractError::InvalidAmount);
        }

        let token_client = require_allowed_token(&env, &loan.token_address)?;
        token_client.transfer(&borrower, &env.current_contract_address(), &payment);

        loan.amount_repaid += payment;

        if loan.amount_repaid >= effective_total_owed {
            // #666/#667: If oracle is configured, hold in escrow pending verification.
            // Otherwise release immediately.
            if cfg.oracle_address.is_some() {
                loan.escrow_status = EscrowStatus::Pending;
                loan.status = LoanStatus::Active; // stays active until oracle releases
                env.storage()
                    .persistent()
                    .set(&DataKey::EscrowAmount(borrower.clone()), &payment);
                env.storage()
                    .persistent()
                    .set(&DataKey::Loan(loan.id), &loan);
                env.events().publish(
                    (symbol_short!("loan"), symbol_short!("escrow")),
                    (borrower, payment),
                );
            } else {
                // No oracle — release immediately
                loan.status = LoanStatus::Repaid;
                loan.repayment_timestamp = Some(now);
                loan.escrow_status = EscrowStatus::Released;

                let vouches: Vec<VouchRecord> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Vouches(borrower.clone()))
                    .unwrap_or(Vec::new(&env));

                let total_stake: i128 = vouches
                    .iter()
                    .filter(|v| v.token == loan.token_address)
                    .map(|v| v.stake)
                    .sum();

                for v in vouches.iter() {
                    if v.token != loan.token_address {
                        continue;
                    }
                    let yield_share = if total_stake > 0 {
                        loan.total_yield * v.stake / total_stake
                    } else {
                        0
                    };
                    token_client.transfer(
                        &env.current_contract_address(),
                        &v.voucher,
                        &(v.stake + yield_share),
                    );
                }

                vouch::process_withdrawal_queue(&env, &borrower);

                env.storage()
                    .persistent()
                    .remove(&DataKey::ActiveLoan(borrower.clone()));
                env.storage()
                    .persistent()
                    .remove(&DataKey::Vouches(borrower.clone()));

                env.events().publish(
                    (symbol_short!("loan"), symbol_short!("repaid")),
                    (borrower.clone(), loan.amount),
                );

                env.storage()
                    .persistent()
                    .set(&DataKey::Loan(loan.id), &loan);
            }
        } else {
            env.storage()
                .persistent()
                .set(&DataKey::Loan(loan.id), &loan);
        }

        Ok(())
    }

    /// #667: Called by the registered oracle to verify a repayment held in escrow.
    /// If `approved` is true, releases funds to vouchers. If false, returns funds to borrower.
    pub fn verify_repayment(
        env: Env,
        oracle: Address,
        borrower: Address,
        approved: bool,
    ) -> Result<(), ContractError> {
        oracle.require_auth();
        require_not_paused(&env)?;

        // Verify caller is the registered oracle
        let cfg = config(&env);
        let registered = cfg.oracle_address.ok_or(ContractError::OracleUnauthorized)?;
        if oracle != registered {
            return Err(ContractError::OracleUnauthorized);
        }

        let loan_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLoan(borrower.clone()))
            .ok_or(ContractError::NoActiveLoan)?;
        let mut loan: LoanRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .ok_or(ContractError::NoActiveLoan)?;

        if loan.escrow_status != EscrowStatus::Pending {
            return Err(ContractError::NoEscrowFound);
        }

        let escrowed: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::EscrowAmount(borrower.clone()))
            .unwrap_or(0);

        let token_client = require_allowed_token(&env, &loan.token_address)?;
        let now = env.ledger().timestamp();

        if approved {
            loan.escrow_status = EscrowStatus::Released;
            loan.status = LoanStatus::Repaid;
            loan.repayment_timestamp = Some(now);

            let vouches: Vec<VouchRecord> = env
                .storage()
                .persistent()
                .get(&DataKey::Vouches(borrower.clone()))
                .unwrap_or(Vec::new(&env));

            let total_stake: i128 = vouches
                .iter()
                .filter(|v| v.token == loan.token_address)
                .map(|v| v.stake)
                .sum();

            for v in vouches.iter() {
                if v.token != loan.token_address {
                    continue;
                }
                // Issue #633: Yield tiering — vouch age bonus.
                // Vouches older than 30 days get +50% of their yield share.
                // Vouches older than 7 days get +25% of their yield share.
                let vouch_age_secs = loan.disbursement_timestamp.saturating_sub(v.vouch_timestamp);
                let age_multiplier_bps: i128 = if vouch_age_secs >= 30 * 24 * 60 * 60 {
                    15_000 // 150%
                } else if vouch_age_secs >= 7 * 24 * 60 * 60 {
                    12_500 // 125%
                } else {
                    10_000 // 100% base
                };

                let base_yield_share = if total_stake > 0 {
                    loan.total_yield * v.stake / total_stake
                } else {
                    0
                };
                let tiered_yield = base_yield_share * age_multiplier_bps / 10_000;

                // Issue #634: Liquidity mining reward on top of yield.
                let cfg = config(&env);
                let mining_reward = if cfg.liquidity_mining_rate_bps > 0 {
                    v.stake * cfg.liquidity_mining_rate_bps / 10_000
                } else {
                    0
                };

                token_client.transfer(
                    &env.current_contract_address(),
                    &v.voucher,
                    &(v.stake + tiered_yield + mining_reward),
                );
            }

            vouch::process_withdrawal_queue(&env, &borrower);

            env.storage()
                .persistent()
                .remove(&DataKey::ActiveLoan(borrower.clone()));
            env.storage()
                .persistent()
                .remove(&DataKey::Vouches(borrower.clone()));

            env.events().publish(
                (symbol_short!("loan"), symbol_short!("repaid")),
                (borrower.clone(), loan.amount),
            );
        } else {
            // Oracle rejected — return escrowed funds to borrower
            loan.escrow_status = EscrowStatus::Rejected;
            loan.amount_repaid -= escrowed;

            if escrowed > 0 {
                token_client.transfer(
                    &env.current_contract_address(),
                    &borrower,
                    &escrowed,
                );
            }

            env.events().publish(
                (symbol_short!("loan"), symbol_short!("escrow_rej")),
                (borrower.clone(), escrowed),
            );
        }

        env.storage()
            .persistent()
            .remove(&DataKey::EscrowAmount(borrower.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);

        Ok(())
    }

    /// #669: Retry a failed repayment. Increments retry_count and re-attempts the transfer.
    /// Returns `MaxRetriesExceeded` if retry_count >= MAX_REPAYMENT_RETRIES.
    pub fn retry_repayment(
        env: Env,
        borrower: Address,
        payment: i128,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;

        let loan_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLoan(borrower.clone()))
            .ok_or(ContractError::NoActiveLoan)?;
        let mut loan: LoanRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Loan(loan_id))
            .ok_or(ContractError::NoActiveLoan)?;

        const MAX_REPAYMENT_RETRIES: u32 = 3;
        if loan.retry_count >= MAX_REPAYMENT_RETRIES {
            return Err(ContractError::MaxRetriesExceeded);
        }

        loan.retry_count += 1;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);

        // Delegate to the standard repay logic
        Self::repay(env, borrower, payment)
    }

    pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
        let loan_id: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ActiveLoan(borrower.clone()))?;
        env.storage().persistent().get(&DataKey::Loan(loan_id))
    }

    pub fn get_vouches(env: Env, borrower: Address) -> Vec<VouchRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Vouches(borrower))
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

    pub fn get_config(env: Env) -> Config {
        config(&env)
    }

    pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
        helper_loan_status(&env, &borrower)
    }

    // ── Governance: slash voting ──────────────────────────────────────────────

    pub fn vote_slash(
        env: Env,
        voucher: Address,
        borrower: Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_slash(env, voucher, borrower, approve)
    }

    pub fn get_slash_vote(env: Env, borrower: Address) -> Option<SlashVoteRecord> {
        governance::get_slash_vote(env, borrower)
    }

    pub fn set_slash_vote_quorum(env: Env, admin_signers: Vec<Address>, quorum_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        governance::set_slash_vote_quorum(&env, quorum_bps);
    }

    pub fn get_slash_vote_quorum(env: Env) -> u32 {
        governance::get_slash_vote_quorum(env)
    }

    pub fn execute_slash_vote(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::execute_slash_vote(env, borrower)
    }

    pub fn execute_pending_slash(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::execute_pending_slash(env, borrower)
    }

    // ── Issue #680: slash threshold governance ────────────────────────────────

    pub fn propose_slash_threshold(
        env: Env,
        proposer: Address,
        new_threshold: i128,
    ) -> Result<u64, ContractError> {
        governance::propose_slash_threshold(env, proposer, new_threshold)
    }

    pub fn vote_slash_threshold(
        env: Env,
        voter: Address,
        proposal_id: u64,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_slash_threshold(env, voter, proposal_id, approve)
    }

    pub fn finalize_slash_threshold(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::finalize_slash_threshold(env, proposal_id)
    }

    pub fn get_slash_threshold_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<SlashThresholdProposal> {
        governance::get_slash_threshold_proposal(env, proposal_id)
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn pause(env: Env, admin_signers: Vec<Address>) {
        admin::pause(env, admin_signers)
    }

    pub fn unpause(env: Env, admin_signers: Vec<Address>) {
        admin::unpause(env, admin_signers)
    }

    pub fn get_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn set_config(env: Env, admin_signers: Vec<Address>, cfg: Config) {
        admin::set_config(env, admin_signers, cfg)
    }

    pub fn update_config(
        env: Env,
        admin_signers: Vec<Address>,
        yield_bps: Option<i128>,
        slash_bps: Option<i128>,
    ) {
        admin::update_config(env, admin_signers, yield_bps, slash_bps)
    }

    // ── Issue #682: multi-sig config updates ──────────────────────────────────

    pub fn propose_config_update(
        env: Env,
        proposer: Address,
        key: ConfigUpdateKey,
        new_value: u32,
    ) -> Result<u64, ContractError> {
        admin::propose_config_update(env, proposer, key, new_value)
    }

    pub fn approve_config_update(
        env: Env,
        admin: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        admin::approve_config_update(env, admin, proposal_id)
    }

    pub fn finalize_config_update(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        admin::finalize_config_update(env, proposal_id)
    }

    pub fn get_config_update_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<ConfigUpdateProposal> {
        admin::get_config_update_proposal(env, proposal_id)
    }

    // ── Issue #683: emergency pause ───────────────────────────────────────────

    pub fn emergency_pause(env: Env, admin: Address) -> Result<(), ContractError> {
        admin::emergency_pause(env, admin)
    }

    pub fn emergency_unpause(env: Env, admin_signers: Vec<Address>) -> Result<(), ContractError> {
        admin::emergency_unpause(env, admin_signers)
    }
}
