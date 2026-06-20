#![no_std]

pub mod admin;
mod contract;
pub mod errors;
pub mod governance;
pub mod helpers;
pub mod insurance;
pub mod loan;
pub mod reputation;
#[cfg(test)]
mod tests;
pub mod types;
pub mod vouch;
pub mod cache;
pub mod error_response;
pub mod versioning;
pub mod proof_validator;
pub mod cross_chain;

pub use errors::ContractError;
pub use types::*;
pub use cross_chain::{BridgeAttestation, BridgeAttestationPayload, CrossChainLoanMetadata, UnifiedReputation};

#[cfg(test)]
mod slash_threshold_voting_test;
#[cfg(test)]
mod slash_cooldown_test;
#[cfg(test)]
mod config_update_voting_test;
#[cfg(test)]
mod referral_test;
#[cfg(test)]
mod bug_condition_test;
#[cfg(test)]
mod coverage_test;

pub use errors::ContractError;
pub use types::*;
mod emergency_pause_test;
#[cfg(test)]
mod withdrawal_queue_test;
#[cfg(test)]
mod cross_chain_vouch_test;
#[cfg(test)]
mod property_stake_loan_invariants_test;
#[cfg(test)]
mod admin_whitelist_blacklist_test;

use crate::helpers::{
    config, get_active_loan_record, has_active_loan, loan_status as helper_loan_status,
    require_allowed_token, require_not_paused,
};
use soroban_sdk::BytesN;

#[contract]
pub struct QuorumCreditContract;

#[contractimpl]
impl QuorumCreditContract {
    // ── Initialization ────────────────────────────────────────────────────────

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

        helpers::validate_admin_config(
            &env,
            &admins,
            admin_threshold,
            &Vec::new(&env),
            &Vec::new(&env),
        )?;
        helpers::require_valid_token(&env, &token)?;

        env.storage().instance().set(&DataKey::Deployer, &deployer);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                admins: admins.clone(),
                admin_threshold,
                admin_whitelist: Vec::new(&env),
                admin_blacklist: Vec::new(&env),
                token,
                allowed_tokens: Vec::new(&env),
                yield_bps: DEFAULT_YIELD_BPS,
                slash_bps: DEFAULT_SLASH_BPS,
                max_vouchers: DEFAULT_MAX_VOUCHERS,
                min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
                loan_duration: DEFAULT_LOAN_DURATION,
                max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
                grace_period: 0,
            },
        );

        env.events().publish(
            (symbol_short!("contract"), symbol_short!("init")),
            (deployer, admins, admin_threshold, token),
        );

        Ok(())
    }

    // ── Vouching ──────────────────────────────────────────────────────────────

                min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
                prepayment_penalty_bps: 0,
                liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
                voting_period_seconds: DEFAULT_VOTING_PERIOD_SECONDS,
                slash_cooldown_seconds: 0,
                emergency_pause_enabled: false,
                early_repayment_discount_bps: 0,
                oracle_address: None,
                slash_delay_seconds: 0,
                successor_admin: None,
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
    ) -> Result<(), ContractError> {
        vouch::vouch(env, voucher, borrower, stake, token)
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

    /// #642: Vouch with an explicit sector label for diversification enforcement.
    pub fn vouch_with_sector(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        sector: String,
    ) -> Result<(), ContractError> {
        vouch::vouch_with_sector(env, voucher, borrower, stake, token, sector)
    }

    pub fn batch_vouch(
        env: Env,
        voucher: Address,
        borrowers: Vec<Address>,
        stakes: Vec<i128>,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::batch_vouch(env, voucher, borrowers, stakes, token)
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
        acquire_lock(&env)?;
        let result = vouch::increase_stake(env.clone(), voucher, borrower, additional);
        release_lock(&env);
        result
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

    pub fn transfer_vouch(
        env: Env,
        from: Address,
        to: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::transfer_vouch(env, from, to, borrower)
    }

    // ── Loans ─────────────────────────────────────────────────────────────────

    pub fn register_referral(
        env: Env,
        borrower: Address,
        referrer: Address,
    ) -> Result<(), ContractError> {
        loan::register_referral(env, borrower, referrer)
    }

    pub fn get_referrer(env: Env, borrower: Address) -> Option<Address> {
        loan::get_referrer(env, borrower)
    }

    pub fn set_referral_bonus_bps(env: Env, admin_signers: Vec<Address>, bonus_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        assert!(bonus_bps <= 10_000, "bonus_bps must not exceed 10000");
        env.storage()
            .instance()
            .set(&DataKey::ReferralBonusBps, &bonus_bps);
    }

    pub fn get_referral_bonus_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ReferralBonusBps)
            .unwrap_or(DEFAULT_REFERRAL_BONUS_BPS)
        acquire_lock(&env)?;
        let result = vouch::decrease_stake(env.clone(), voucher, borrower, amount);
        release_lock(&env);
        result
    }

    pub fn withdraw_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::withdraw_vouch(env.clone(), voucher, borrower);
        release_lock(&env);
        result
    }

    pub fn request_withdrawal(
        env: Env,
        voucher: Address,
        borrower: Address,
        priority_fee: i128,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::request_withdrawal(env.clone(), voucher, borrower, priority_fee);
        release_lock(&env);
        result
    }

    pub fn partial_withdraw(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        acquire_lock(&env)?;
        let result = vouch::partial_withdraw(env.clone(), voucher, borrower);
        release_lock(&env);
        result
    }

    pub fn get_withdrawal_queue(env: Env, borrower: Address) -> Vec<QueuedWithdrawal> {
        vouch::get_withdrawal_queue(env, borrower)
    }

    pub fn process_withdrawal_batch(env: Env, borrower: Address, count: u32) -> u32 {
        vouch::process_withdrawal_batch(&env, &borrower, count)
    }

    pub fn request_loan(
        env: Env,
        borrower: Address,
        amount: i128,
        threshold: i128,
        loan_purpose: soroban_sdk::String,
        token: Address,
    ) -> Result<(), ContractError> {
        loan::request_loan(env, borrower, amount, threshold, loan_purpose, token)
    }

    pub fn dispute_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        evidence_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        vouch::dispute_vouch(env, voucher, borrower, evidence_hash)
    }

    pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
        loan::repay(env, borrower, payment)
    }

    /// Admin marks a loan defaulted; slash_bps% of each voucher's stake is slashed.
    pub fn slash(env: Env, admin_signers: Vec<Address>, borrower: Address) {
        helpers::require_admin_approval(&env, &admin_signers);
        helpers::require_not_paused(&env).expect("contract is paused");

        let mut loan = helpers::get_active_loan_record(&env, &borrower)
            .expect("no active loan");

        if loan.repaid || loan.defaulted {
            panic_with_error!(&env, ContractError::NoActiveLoan);
        }

        let cfg = config(&env);
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        loan.defaulted = true;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));

        let token_client = token::Client::new(&env, &loan.token_address);
        let mut total_slashed: i128 = 0;
        for v in vouches.iter() {
            let slash_amount = v.stake * cfg.slash_bps / 10_000;
            let returned = v.stake - slash_amount;
            if returned > 0 {
                token_client.transfer(&env.current_contract_address(), &v.voucher, &returned);
            }
            total_slashed += slash_amount;
        }

        helpers::add_slash_balance(&env, total_slashed);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DefaultCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::DefaultCount(borrower.clone()), &(count + 1));

        if let Some(nft_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationNft)
        {
            ReputationNftExternalClient::new(&env, &nft_addr).burn(&borrower);
        }
        loan_purpose: String,
        token_addr: Address,
    ) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;
        acquire_lock(&env)?;

        if has_active_loan(&env, &borrower) {
            release_lock(&env);
            return Err(ContractError::ActiveLoanExists);
        }

        let token_client = require_allowed_token(&env, &token_addr)?;
        let cfg = config(&env);

        if amount < cfg.min_loan_amount {
            release_lock(&env);
            return Err(ContractError::LoanBelowMinAmount);
        }

        if amount <= 0 {
            release_lock(&env);
            return Err(ContractError::InvalidAmount);
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
            release_lock(&env);
            return Err(ContractError::InsufficientFunds);
        }

        // #643: Validate loan_purpose against allowed_purposes whitelist (empty = all allowed)
        if !cfg.allowed_purposes.is_empty() {
            let purpose_allowed = cfg.allowed_purposes.iter().any(|p| p == loan_purpose);
            if !purpose_allowed {
                return Err(ContractError::LoanPurposeNotAllowed);
            }
        }

        // #642: Enforce sector diversification — no single sector may contribute > 50% of total stake
        if total_stake > 0 {
            let mut sector_names: Vec<soroban_sdk::String> = Vec::new(&env);
            let mut sector_amounts: Vec<i128> = Vec::new(&env);
            for v in vouches.iter() {
                if v.token != token_addr {
                    continue;
                }
                let mut found = false;
                for i in 0..sector_names.len() {
                    if sector_names.get(i).unwrap() == v.sector {
                        let cur = sector_amounts.get(i).unwrap();
                        sector_amounts.set(i, cur + v.stake);
                        found = true;
                        break;
                    }
                }
                if !found {
                    sector_names.push_back(v.sector.clone());
                    sector_amounts.push_back(v.stake);
                }
            }
            for i in 0..sector_amounts.len() {
                let s_stake = sector_amounts.get(i).unwrap();
                if s_stake * 2 > total_stake {
                    return Err(ContractError::SectorConcentrationTooHigh);
                }
            }
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

        // #644: Collect insurance premium from borrower if configured
        if cfg.insurance_premium_bps > 0 {
            let premium = amount * cfg.insurance_premium_bps / 10_000;
            if premium > 0 {
                token_client.transfer(&borrower, &env.current_contract_address(), &premium);
                let pool_balance: i128 = env
                    .storage()
                    .instance()
                    .get(&DataKey::InsurancePool)
                    .unwrap_or(0);
                env.storage()
                    .instance()
                    .set(&DataKey::InsurancePool, &(pool_balance + premium));
            }
        }

        token_client.transfer(&env.current_contract_address(), &borrower, &amount);

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("created")),
            (borrower, amount),
        );

        release_lock(&env);
        Ok(())
    }

    /// Confirm intent to repay the active loan.
    ///
    /// When `Config.confirmation_required` is `true`, borrowers must call this
    /// function before calling `repay`. The confirmation is stored per-loan and
    /// consumed on the first successful `repay` call, so it cannot be replayed.
    ///
    /// This is a no-op (succeeds silently) when `confirmation_required` is false,
    /// so callers can always call it without checking the config first.
    pub fn confirm_repayment(env: Env, borrower: Address) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;

        let loan = get_active_loan_record(&env, &borrower)?;

        env.storage()
            .persistent()
            .set(&DataKey::RepaymentConfirmation(loan.id), &true);

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("repay_ok")),
            (borrower, loan.id),
        );

        Ok(())
    }

    pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
        borrower.require_auth();
        require_not_paused(&env)?;
        acquire_lock(&env)?;

        let mut loan = match get_active_loan_record(&env, &borrower) {
            Ok(l) => l,
            Err(e) => { release_lock(&env); return Err(e); }
        };

        if let Err(e) = validate_amount(&env, payment) {
            release_lock(&env);
            return Err(e);
        }

        let cfg = config(&env);

        // If confirmation_required is enabled, the borrower must have called
        // confirm_repayment first. The confirmation is keyed by loan_id and
        // consumed here so it cannot be replayed.
        if cfg.confirmation_required {
            let confirmed: bool = env
                .storage()
                .persistent()
                .get(&DataKey::RepaymentConfirmation(loan.id))
                .unwrap_or(false);
            if !confirmed {
                return Err(ContractError::RepaymentNotConfirmed);
            }
            // Consume the confirmation — one-time use.
            env.storage()
                .persistent()
                .remove(&DataKey::RepaymentConfirmation(loan.id));
        }

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
            release_lock(&env);
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
                    v.stake * cfg.liquidity_mining_rate_bps as i128 / 10_000
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
                (symbol_short!("loan"), symbol_short!("escrw_rej")),
                (borrower.clone(), escrowed),
            );
        }

        env.storage()
            .persistent()
            .remove(&DataKey::EscrowAmount(borrower.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);

        release_lock(&env);
        Ok(())
    }

    /// Callable by anyone after the loan deadline has passed. Applies the standard slash penalty.
    pub fn auto_slash(env: Env, borrower: Address) {
        let mut loan = helpers::get_active_loan_record(&env, &borrower)
            .expect("no active loan");

        if loan.repaid || loan.defaulted {
            panic_with_error!(&env, ContractError::NoActiveLoan);
        }
        assert!(
            env.ledger().timestamp() > loan.deadline,
            "loan deadline has not passed"
        );

        let cfg = config(&env);
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        loan.defaulted = true;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));

        let token_client = token::Client::new(&env, &loan.token_address);
        let mut total_slash: i128 = 0;
        for v in vouches.iter() {
            let slash_amount = v.stake * cfg.slash_bps / 10_000;
            let returned = v.stake - slash_amount;
            total_slash += slash_amount;
            if returned > 0 {
                token_client.transfer(&env.current_contract_address(), &v.voucher, &returned);
            }
        }

        helpers::add_slash_balance(&env, total_slash);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DefaultCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::DefaultCount(borrower.clone()), &(count + 1));

        if let Some(nft_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationNft)
        {
            ReputationNftExternalClient::new(&env, &nft_addr).burn(&borrower);
        }
    }

    /// Allows vouchers to claim back their stake if loan has expired without repayment or slash.
    pub fn claim_expired_loan(env: Env, borrower: Address) {
        borrower.require_auth();

        let mut loan = helpers::get_active_loan_record(&env, &borrower)
            .expect("no active loan");

        if loan.repaid || loan.defaulted {
            panic_with_error!(&env, ContractError::NoActiveLoan);
        }

        let now = env.ledger().timestamp();
        assert!(now >= loan.deadline, "loan has not expired yet");

        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        let token_client = token::Client::new(&env, &loan.token_address);
        for v in vouches.iter() {
            token_client.transfer(&env.current_contract_address(), &v.voucher, &v.stake);
        }

        loan.defaulted = true;
        env.storage()
            .persistent()
            .set(&DataKey::Loan(loan.id), &loan);
        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower));
    }

    /// Admin withdraws accumulated slashed funds to a recipient address.
    pub fn slash_treasury(env: Env, admin_signers: Vec<Address>, recipient: Address) {
        helpers::require_admin_approval(&env, &admin_signers);

        let amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SlashTreasury)
            .unwrap_or(0);
        assert!(amount > 0, "no slashed funds to withdraw");

        env.storage().instance().set(&DataKey::SlashTreasury, &0i128);
        helpers::token(&env).transfer(&env.current_contract_address(), &recipient, &amount);
    }

    // ── Loan Pool ─────────────────────────────────────────────────────────────

    /// Admin function: atomically disburse a batch of small loans to multiple borrowers.
    pub fn create_loan_pool(
        env: Env,
        admin_signers: Vec<Address>,
        borrowers: Vec<Address>,
        amounts: Vec<i128>,
    ) -> Result<u64, ContractError> {
        helpers::require_admin_approval(&env, &admin_signers);

        if borrowers.len() != amounts.len() {
            return Err(ContractError::PoolLengthMismatch);
        }
        if borrowers.is_empty() {
            return Err(ContractError::PoolEmpty);
        }

        let cfg = config(&env);
        let now = env.ledger().timestamp();
        let deadline = now + cfg.loan_duration;

        let mut total_amount: i128 = 0;
        for i in 0..borrowers.len() {
            let borrower = borrowers.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            assert!(
                amount >= cfg.min_loan_amount,
                "pool: amount below minimum loan threshold"
            );

            if helpers::has_active_loan(&env, &borrower) {
                return Err(ContractError::PoolBorrowerActiveLoan);
            }

            let total_stake: i128 = env
                .storage()
                .persistent()
                .get::<DataKey, Vec<VouchRecord>>(&DataKey::Vouches(borrower.clone()))
                .unwrap_or(Vec::new(&env))
                .iter()
                .map(|v| v.stake)
                .sum();
            let max_allowed = total_stake * cfg.max_loan_to_stake_ratio as i128 / 100;
            assert!(
                amount <= max_allowed,
                "pool: loan amount exceeds maximum collateral ratio for borrower"
            );

            total_amount += amount;
        }

        let token_client = helpers::token(&env);
        let contract_balance = token_client.balance(&env.current_contract_address());
        if contract_balance < total_amount {
            return Err(ContractError::PoolInsufficientFunds);
        }

        let pool_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LoanPoolCounter)
            .unwrap_or(0u64)
            .checked_add(1)
            .expect("pool ID overflow");
        env.storage()
            .instance()
            .set(&DataKey::LoanPoolCounter, &pool_id);

        for i in 0..borrowers.len() {
            let borrower = borrowers.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
            let loan_id = helpers::next_loan_id(&env);

            env.storage().persistent().set(
                &DataKey::Loan(loan_id),
                &LoanRecord {
                    id: loan_id,
                    borrower: borrower.clone(),
                    co_borrowers: Vec::new(&env),
                    amount,
                    amount_repaid: 0,
                    total_yield: amount * cfg.yield_bps / 10_000,
                    repaid: false,
                    defaulted: false,
                    created_at: now,
                    disbursement_timestamp: now,
                    repayment_timestamp: None,
                    deadline,
                    loan_purpose: soroban_sdk::String::from_str(&env, "pool"),
                    token_address: cfg.token.clone(),
                },
            );
            env.storage()
                .persistent()
                .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
            env.storage()
                .persistent()
                .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

            token_client.transfer(&env.current_contract_address(), &borrower, &amount);

            env.events().publish(
                (symbol_short!("pool"), symbol_short!("loan")),
                (pool_id, borrower.clone(), amount, deadline),
            );
        }

        env.storage().persistent().set(
            &DataKey::LoanPool(pool_id),
            &LoanPoolRecord {
                pool_id,
                borrowers: borrowers.clone(),
                amounts: amounts.clone(),
                created_at: now,
                total_disbursed: total_amount,
            },
        );

        env.events().publish(
            (symbol_short!("pool"), symbol_short!("created")),
            (pool_id, borrowers.len(), total_amount),
        );

        Ok(pool_id)
    }

    pub fn get_loan_pool(env: Env, pool_id: u64) -> Option<LoanPoolRecord> {
        env.storage().persistent().get(&DataKey::LoanPool(pool_id))
    }

    pub fn get_loan_pool_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::LoanPoolCounter)
            .unwrap_or(0)
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
        admin::add_admin(env, admin_signers, new_admin)
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

    // ── Admin management ─────────────────────────────────────────────────────

    pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
        admin::add_admin(env, admin_signers, new_admin)
    }

    pub fn remove_admin(env: Env, admin_signers: Vec<Address>, admin_to_remove: Address) {
        admin::remove_admin(env, admin_signers, admin_to_remove)
    }

    pub fn set_admin_threshold(env: Env, admin_signers: Vec<Address>, new_threshold: u32) {
        admin::set_admin_threshold(env, admin_signers, new_threshold)
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

    // ── Issue #688: Admin whitelist management ────────────────────────────────

    pub fn add_to_admin_whitelist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::add_to_admin_whitelist(env, admin_signers, address)
    }

    pub fn remove_from_admin_whitelist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::remove_from_admin_whitelist(env, admin_signers, address)
    }

    // ── Issue #689: Admin blacklist management ────────────────────────────────

    pub fn add_to_admin_blacklist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::add_to_admin_blacklist(env, admin_signers, address)
    }

    pub fn remove_from_admin_blacklist(env: Env, admin_signers: Vec<Address>, address: Address) {
        admin::remove_from_admin_blacklist(env, admin_signers, address)
    }

    pub fn update_config(
        env: Env,
        admin_signers: Vec<Address>,
        yield_bps: Option<i128>,
        slash_bps: Option<i128>,
    ) {
        admin::update_config(env, admin_signers, yield_bps, slash_bps)
    }

    pub fn set_reputation_nft(env: Env, admin_signers: Vec<Address>, nft_contract: Address) {
        admin::set_reputation_nft(env, admin_signers, nft_contract)
    }

    pub fn set_min_stake(env: Env, admin_signers: Vec<Address>, amount: i128) {
        admin::set_min_stake(env, admin_signers, amount)
    }

    pub fn set_max_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) {
        admin::set_max_loan_amount(env, admin_signers, amount)
    }

    pub fn set_min_vouchers(env: Env, admin_signers: Vec<Address>, count: u32) {
        admin::set_min_vouchers(env, admin_signers, count)
    }

    pub fn set_max_loan_to_stake_ratio(env: Env, admin_signers: Vec<Address>, ratio: u32) {
        admin::set_max_loan_to_stake_ratio(env, admin_signers, ratio)
    }

    pub fn set_max_vouchers_per_loan(env: Env, admin_signers: Vec<Address>, max: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        assert!(max > 0, "max_vouchers_per_loan must be greater than zero");
        let mut cfg = config(&env);
        cfg.max_vouchers = max;
        env.storage().instance().set(&DataKey::Config, &cfg);
    }

    pub fn add_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
        admin::add_allowed_token(env, admin_signers, token)
    }

    pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
        admin::remove_allowed_token(env, admin_signers, token)
    }

    pub fn set_slash_vote_quorum(env: Env, admin_signers: Vec<Address>, quorum_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        governance::set_slash_vote_quorum(&env, quorum_bps);
    }

    // ── Governance ────────────────────────────────────────────────────────────

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

    pub fn get_slash_vote_quorum(env: Env) -> u32 {
        governance::get_slash_vote_quorum(env)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn get_token(env: Env) -> Address {
        config(&env).token
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        admin::get_admins(env)
    }

    pub fn get_admin_threshold(env: Env) -> u32 {
        admin::get_admin_threshold(env)
    }

    pub fn get_slash_treasury_balance(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::SlashTreasury)
            .unwrap_or(0)
    }

    pub fn get_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
        loan::loan_status(env, borrower)
    }

    pub fn vouch_exists(env: Env, voucher: Address, borrower: Address) -> bool {
        vouch::vouch_exists(env, voucher, borrower)
    }

    pub fn is_whitelisted(env: Env, voucher: Address) -> bool {
        admin::is_whitelisted(env, voucher)
    }

    pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
        loan::get_loan(env, borrower)
    }

    pub fn get_loan_by_id(env: Env, loan_id: u64) -> Option<LoanRecord> {
        loan::get_loan_by_id(env, loan_id)
    }

    pub fn get_vouches(env: Env, borrower: Address) -> Option<Vec<VouchRecord>> {
        env.storage().persistent().get(&DataKey::Vouches(borrower))
    }

    pub fn is_eligible(env: Env, borrower: Address, threshold: i128) -> bool {
        loan::is_eligible(env, borrower, threshold)
    }

    pub fn get_contract_balance(env: Env) -> i128 {
        helpers::token(&env).balance(&env.current_contract_address())
    }

    pub fn voucher_history(env: Env, voucher: Address) -> Vec<Address> {
        vouch::voucher_history(env, voucher)
    }

    pub fn get_reputation(env: Env, borrower: Address) -> u32 {
        let nft_addr: Address = match env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationNft)
        {
            Some(a) => a,
            None => return 0,
        };
        ReputationNftExternalClient::new(&env, &nft_addr).balance(&borrower)
    }

    pub fn total_vouched(env: Env, borrower: Address) -> Result<i128, ContractError> {
        vouch::total_vouched(env, borrower)
    }

    pub fn repayment_count(env: Env, borrower: Address) -> u32 {
        loan::repayment_count(env, borrower)
    }

    pub fn loan_count(env: Env, borrower: Address) -> u32 {
        loan::loan_count(env, borrower)
    }

    pub fn default_count(env: Env, borrower: Address) -> u32 {
        loan::default_count(env, borrower)
    }

    pub fn get_protocol_fee(env: Env) -> u32 {
        admin::get_protocol_fee(env)
    }

    pub fn get_fee_treasury(env: Env) -> Option<Address> {
        admin::get_fee_treasury(env)
    }

    pub fn is_blacklisted(env: Env, borrower: Address) -> bool {
        admin::is_blacklisted(env, borrower)
    }

    pub fn get_min_stake(env: Env) -> i128 {
        admin::get_min_stake(env)
    }

    pub fn get_max_loan_amount(env: Env) -> i128 {
        admin::get_max_loan_amount(env)
    }

    pub fn get_min_vouchers(env: Env) -> u32 {
        admin::get_min_vouchers(env)
    }

    pub fn get_max_loan_to_stake_ratio(env: Env) -> u32 {
        admin::get_max_loan_to_stake_ratio(env)
    }

    pub fn get_max_vouchers_per_loan(env: Env) -> u32 {
        config(&env).max_vouchers
    }

    pub fn get_config(env: Env) -> Config {
        admin::get_config(env)
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

    /// Toggle the borrower repayment confirmation requirement on/off.
    ///
    /// When enabled, borrowers must call `confirm_repayment` before `repay`.
    pub fn set_confirmation_required(
        env: Env,
        admin_signers: Vec<Address>,
        enabled: bool,
    ) {
        admin::set_confirmation_required(env, admin_signers, enabled)
    }

    pub fn set_successor_admin(
        env: Env,
        admin_signers: Vec<Address>,
        successor: Option<Address>,
    ) {
        admin::set_successor_admin(env, admin_signers, successor)
    }

    pub fn claim_successor_admin(env: Env) -> Result<(), ContractError> {
        admin::claim_successor_admin(env)
    }

    // ── Issue #14: Cross-chain loan portability ───────────────────────────────

    pub fn set_bridge_public_key(
        env: Env,
        admin_signers: Vec<Address>,
        origin_chain: u32,
        public_key: soroban_sdk::BytesN<32>,
    ) -> Result<(), ContractError> {
        cross_chain::set_bridge_public_key(env, admin_signers, origin_chain, public_key)
    }

    pub fn validate_bridge_attestation(
        env: Env,
        metadata: CrossChainLoanMetadata,
        attestation: BridgeAttestation,
    ) -> Result<(), ContractError> {
        cross_chain::validate_bridge_attestation(env, metadata, attestation)
    }

    pub fn bridge_attestation_message(
        env: Env,
        metadata: CrossChainLoanMetadata,
        nonce: u64,
        timestamp: u64,
    ) -> soroban_sdk::Bytes {
        cross_chain::bridge_attestation_message(&env, &metadata, nonce, timestamp)
    }

    pub fn mirror_loan_to_chain(
        env: Env,
        metadata: CrossChainLoanMetadata,
        attestation: BridgeAttestation,
    ) -> Result<(), ContractError> {
        cross_chain::mirror_loan_to_chain(env, metadata, attestation)
    }

    pub fn query_reputation_cross_chain(
        env: Env,
        borrower: Address,
    ) -> Option<UnifiedReputation> {
        cross_chain::query_reputation_cross_chain(env, borrower)
    }

    pub fn query_mirrored_loan(
        env: Env,
        origin_chain: u32,
        loan_id: u64,
    ) -> Option<CrossChainLoanMetadata> {
        cross_chain::query_mirrored_loan(env, origin_chain, loan_id)
    }

    pub fn is_bridge_nonce_used(env: Env, origin_chain: u32, nonce: u64) -> bool {
        cross_chain::is_bridge_nonce_used(env, origin_chain, nonce)
    }
}
