#![no_std]

use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, Vec,
};

pub mod admin;
pub mod benchmarks;
pub mod errors;
pub mod fraud_detection;
pub mod governance;
pub mod health;
pub mod helpers;
pub mod liquidity_mining;
pub mod loan;
pub mod reputation;
pub mod staking_derivatives;
pub mod types;
pub mod upgrade;
pub mod vouch;
pub mod vouch_snapshot;

#[cfg(test)]
mod admin_audit_log_test;
#[cfg(test)]
mod benchmarks_test;
#[cfg(test)]
mod admin_key_rotation_test;
#[cfg(test)]
mod admin_timelock_test;
#[cfg(test)]
mod governance_token_voting_test;
#[cfg(test)]
mod bug_condition_test;
#[cfg(test)]
mod borrower_whitelist_test;
#[cfg(test)]
mod config_bps_test;
#[cfg(test)]
mod double_repay_test;
#[cfg(test)]
mod duplicate_loan_test;
#[cfg(test)]
mod duplicate_vouch_test;
#[cfg(test)]
mod get_loan_status_test;
#[cfg(test)]
mod governance_test;
#[cfg(test)]
mod grace_period_test;
#[cfg(test)]
mod health_check_test;
#[cfg(test)]
mod initialize_test;
#[cfg(test)]
mod invalid_bps_test;
#[cfg(test)]
mod loan_purpose_query_test;
#[cfg(test)]
mod loan_purpose_test;
#[cfg(test)]
mod max_loan_amount_test;
#[cfg(test)]
mod multi_asset_test;
#[cfg(test)]
mod referral_test;
#[cfg(test)]
mod repay_overpayment_test;
#[cfg(test)]
mod request_loan_insufficient_stake_test;
#[cfg(test)]
mod security_fixes_test;
#[cfg(test)]
mod set_min_loan_amount_test;
#[cfg(test)]
mod simple_double_repay_test;
#[cfg(test)]
mod token_config_test;
#[cfg(test)]
mod upgrade_validation_test;
#[cfg(test)]
mod vouch_min_stake_test;
#[cfg(test)]
mod vouch_zero_stake_test;
#[cfg(test)]
mod voucher_whitelist_test;
#[cfg(test)]
mod voucher_stake_limit_test;
#[cfg(test)]
mod vouch_cooldown_test;
#[cfg(test)]
mod decrease_stake_full_withdrawal_test;
#[cfg(test)]
mod initialize_admin_threshold_test;
#[cfg(test)]
mod invariants_test;
#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod syndication_test;
#[cfg(test)]
mod default_prediction_test;

pub use errors::ContractError;
pub use types::*;

use helpers::{require_valid_token, validate_admin_config};
use reputation::ReputationNftExternalClient;

#[contract]
pub struct QuorumCreditContract;

#[contractimpl]
impl QuorumCreditContract {
    /// One-time contract initialization. Deployer must sign.
    pub fn initialize(
        env: Env,
        deployer: Address,
        admins: Vec<Address>,
        admin_threshold: u32,
        token: Address,
    ) -> Result<(), ContractError> {
        deployer.require_auth();

        if env.storage().instance().has(&DataKey::Config) {
            panic_with_error!(&env, ContractError::AlreadyInitialized);
        }

        validate_admin_config(&env, &admins, admin_threshold)?;
        require_valid_token(&env, &token)?;

        env.storage().instance().set(&DataKey::Deployer, &deployer);
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                admins: admins.clone(),
                admin_threshold,
                token: token.clone(),
                allowed_tokens: Vec::new(&env),
                yield_bps: DEFAULT_YIELD_BPS,
                slash_bps: DEFAULT_SLASH_BPS,
                max_vouchers: DEFAULT_MAX_VOUCHERS,
                min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
                loan_duration: DEFAULT_LOAN_DURATION,
                max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
                grace_period: 0,
                liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
            },
        );

        env.events().publish(
            (symbol_short!("contract"), symbol_short!("init")),
            (deployer, admins.clone(), admin_threshold, token.clone()),
        );
        Ok(())
    }

    // ── Vouch ─────────────────────────────────────────────────────────────────

    pub fn vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::vouch(env, voucher, borrower, stake, token)
    }

    // #642: vouch with sector for diversification enforcement
    pub fn vouch_with_sector(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
        sector: soroban_sdk::String,
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

    pub fn transfer_vouch(
        env: Env,
        from: Address,
        to: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::transfer_vouch(env, from, to, borrower)
    }

    // ── Loan ──────────────────────────────────────────────────────────────────

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
            .unwrap_or(crate::types::DEFAULT_REFERRAL_BONUS_BPS)
    }

    pub fn request_loan(
        env: Env,
        borrower: Address,
        amount: i128,
        threshold: i128,
        loan_purpose: soroban_sdk::String,
        token: Address,
        syndicate_id: Option<u64>,
    ) -> Result<(), ContractError> {
        loan::request_loan(env, borrower, amount, threshold, loan_purpose, token, syndicate_id)
    }

    pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
        loan::repay(env, borrower, payment)
    }

    // Task 1: Loan Cancellation
    pub fn cancel_loan(env: Env, borrower: Address) -> Result<(), ContractError> {
        loan::cancel_loan(env, borrower)
    }

    // Task 2: Large Loan Multi-Signature
    pub fn request_large_loan(
        env: Env,
        borrower: Address,
        amount: i128,
        threshold: i128,
        loan_purpose: soroban_sdk::String,
        loan_category: LoanCategory,
        token: Address,
    ) -> Result<(), ContractError> {
        loan::request_large_loan(env, borrower, amount, threshold, loan_purpose, loan_category, token)
    }

    pub fn approve_large_loan(
        env: Env,
        admin: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        loan::approve_large_loan(env, admin, borrower)
    }

    pub fn execute_large_loan(env: Env, borrower: Address) -> Result<(), ContractError> {
        loan::execute_large_loan(env, borrower)
    }

    // Task 4: Loan Category Analytics
    pub fn get_loans_by_category(env: Env, category: LoanCategory) -> Vec<u64> {
        loan::get_loans_by_category(env, category)
    }

    /// #647: Get all loan IDs in a syndicate.
    pub fn get_syndicate_loans(env: Env, syndicate_id: u64) -> Vec<u64> {
        loan::get_syndicate_loans(env, syndicate_id)
    }

    /// #647: Create a new syndicate pool and return its ID.
    pub fn create_syndicate(env: Env) -> u64 {
        loan::create_syndicate(env)
    }

    /// #646: Get the risk score for a borrower (0..10_000).
    pub fn get_risk_score(env: Env, borrower: Address) -> i128 {
        loan::get_risk_score(env, borrower)
    }

    /// #646: Preview the dynamic yield rate (bps) for a borrower based on their history.
    pub fn get_dynamic_yield_bps(env: Env, borrower: Address) -> i128 {
        loan::get_dynamic_yield_bps(env, borrower)
    }

    /// #646: Preview the dynamic slash rate (bps) for a borrower based on their history.
    pub fn get_dynamic_slash_bps(env: Env, borrower: Address) -> i128 {
        loan::get_dynamic_slash_bps(env, borrower)
    }

    // ── Admin Functions (require admin_threshold signatures) ──────────────────

    pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
        admin::add_admin(env, admin_signers, new_admin)
    }

    pub fn remove_admin(env: Env, admin_signers: Vec<Address>, admin_to_remove: Address) {
        admin::remove_admin(env, admin_signers, admin_to_remove)
    }

    pub fn rotate_admin(
        env: Env,
        admin_signers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) {
        admin::rotate_admin(env, admin_signers, old_admin, new_admin)
    }

    pub fn set_admin_threshold(env: Env, admin_signers: Vec<Address>, new_threshold: u32) {
        admin::set_admin_threshold(env, admin_signers, new_threshold)
    }

    pub fn set_protocol_fee(env: Env, admin_signers: Vec<Address>, fee_bps: u32) {
        admin::set_protocol_fee(env, admin_signers, fee_bps)
    }

    pub fn whitelist_voucher(env: Env, admin_signers: Vec<Address>, voucher: Address) {
        admin::whitelist_voucher(env, admin_signers, voucher)
    }

    pub fn add_voucher_to_whitelist(env: Env, admin_signers: Vec<Address>, voucher: Address) {
        admin::add_voucher_to_whitelist(env, admin_signers, voucher)
    }

    pub fn remove_voucher_from_whitelist(env: Env, admin_signers: Vec<Address>, voucher: Address) {
        admin::remove_voucher_from_whitelist(env, admin_signers, voucher)
    }

    pub fn enable_voucher_whitelist(env: Env, admin_signers: Vec<Address>) {
        admin::enable_voucher_whitelist(env, admin_signers)
    }

    pub fn disable_voucher_whitelist(env: Env, admin_signers: Vec<Address>) {
        admin::disable_voucher_whitelist(env, admin_signers)
    }

    pub fn add_borrower_to_whitelist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
        admin::add_borrower_to_whitelist(env, admin_signers, borrower)
    }

    pub fn remove_borrower_from_whitelist(
        env: Env,
        admin_signers: Vec<Address>,
        borrower: Address,
    ) {
        admin::remove_borrower_from_whitelist(env, admin_signers, borrower)
    }

    pub fn enable_borrower_whitelist(env: Env, admin_signers: Vec<Address>) {
        admin::enable_borrower_whitelist(env, admin_signers)
    }

    pub fn disable_borrower_whitelist(env: Env, admin_signers: Vec<Address>) {
        admin::disable_borrower_whitelist(env, admin_signers)
    }

    pub fn set_fee_treasury(env: Env, admin_signers: Vec<Address>, treasury: Address) {
        admin::set_fee_treasury(env, admin_signers, treasury)
    }

    pub fn upgrade(env: Env, admin_signers: Vec<Address>, new_wasm_hash: BytesN<32>) {
        admin::upgrade(env, admin_signers, new_wasm_hash)
    }

    pub fn pause(env: Env, admin_signers: Vec<Address>) {
        admin::pause(env, admin_signers)
    }

    pub fn unpause(env: Env, admin_signers: Vec<Address>) {
        admin::unpause(env, admin_signers)
    }

    // Task 1: Granular pause functions
    pub fn pause_function(
        env: Env,
        admin_signers: Vec<Address>,
        function_name: soroban_sdk::String,
    ) -> Result<(), ContractError> {
        admin::pause_function(env, admin_signers, function_name)
    }

    pub fn unpause_function(
        env: Env,
        admin_signers: Vec<Address>,
        function_name: soroban_sdk::String,
    ) -> Result<(), ContractError> {
        admin::unpause_function(env, admin_signers, function_name)
    }

    pub fn get_pause_status(env: Env, function_name: soroban_sdk::String) -> bool {
        admin::get_pause_status(env, function_name)
    }

    // Task 3: Co-borrower support
    pub fn request_loan_with_co_borrowers(
        env: Env,
        borrower: Address,
        amount: i128,
        threshold: i128,
        loan_purpose: soroban_sdk::String,
        token: Address,
        co_borrowers: Vec<Address>,
    ) -> Result<(), ContractError> {
        loan::request_loan_with_co_borrowers(env, borrower, amount, threshold, loan_purpose, token, co_borrowers)
    }

    // Task 4: Dispute mechanism
    pub fn dispute_slash(
        env: Env,
        borrower: Address,
        evidence_hash: soroban_sdk::String,
    ) -> Result<u64, ContractError> {
        governance::dispute_slash(env, borrower, evidence_hash)
    }

    pub fn vote_dispute(
        env: Env,
        voucher: Address,
        dispute_id: u64,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_dispute(env, voucher, dispute_id, approve)
    }

    pub fn resolve_dispute(env: Env, dispute_id: u64) -> Result<(), ContractError> {
        governance::resolve_dispute(env, dispute_id)
    }

    pub fn get_dispute(env: Env, dispute_id: u64) -> Option<crate::types::DisputeRecord> {
        governance::get_dispute(env, dispute_id)
    }

    pub fn set_dispute_window(env: Env, admin_signers: Vec<Address>, window_secs: u64) {
        governance::set_dispute_window(env, admin_signers, window_secs)
    }

    pub fn get_dispute_window(env: Env) -> u64 {
        governance::get_dispute_window(env)
    }

    pub fn blacklist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
        admin::blacklist(env, admin_signers, borrower)
    }

    pub fn set_config(env: Env, admin_signers: Vec<Address>, config: Config) {
        admin::set_config(env, admin_signers, config)
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

    pub fn set_min_loan_amount(
        env: Env,
        admin_signers: Vec<Address>,
        amount: i128,
    ) -> Result<(), ContractError> {
        admin::set_min_loan_amount(env, admin_signers, amount)
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

    pub fn set_grace_period(env: Env, admin_signers: Vec<Address>, period: u64) {
        admin::set_grace_period(env, admin_signers, period)
    }

    pub fn add_allowed_token(
        env: Env,
        admin_signers: Vec<Address>,
        token: Address,
    ) -> Result<(), ContractError> {
        admin::add_allowed_token(env, admin_signers, token)
    }

    pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
        admin::remove_allowed_token(env, admin_signers, token)
    }

    pub fn set_token_config(
        env: Env,
        admin_signers: Vec<Address>,
        token: Address,
        token_cfg: TokenConfig,
    ) {
        admin::set_token_config(env, admin_signers, token, token_cfg)
    }

    pub fn get_token_config(env: Env, token: Address) -> Option<TokenConfig> {
        admin::get_token_config(env, token)
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

    pub fn set_slash_vote_quorum(env: Env, admin_signers: Vec<Address>, quorum_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        governance::set_slash_vote_quorum(&env, quorum_bps);
    }

    pub fn get_slash_vote_quorum(env: Env) -> u32 {
        governance::get_slash_vote_quorum(&env)
    }

    pub fn execute_slash_vote(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::execute_slash_vote(env, borrower)
    }

    pub fn propose_admin(
        env: Env,
        admin_signers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        admin::propose_admin(env, admin_signers, new_admin)
    }

    pub fn accept_admin(env: Env) -> Result<(), ContractError> {
        admin::accept_admin(env)
    }

    pub fn propose_slash(
        env: Env,
        proposer: Address,
        borrower: Address,
        delay_secs: u64,
    ) -> Result<u64, ContractError> {
        governance::propose_slash(env, proposer, borrower, delay_secs)
    }

    pub fn execute_slash_proposal(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::execute_slash_proposal(env, proposal_id)
    }

    pub fn cancel_slash_proposal(
        env: Env,
        caller: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        governance::cancel_slash_proposal(env, caller, proposal_id)
    }

    pub fn get_timelock_proposal(env: Env, proposal_id: u64) -> Option<TimelockProposal> {
        governance::get_timelock_proposal(env, proposal_id)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn get_token(env: Env) -> Address {
        helpers::config(&env).token
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

    pub fn is_voucher_whitelisted(env: Env, voucher: Address) -> bool {
        admin::is_whitelisted(env, voucher)
    }

    pub fn is_voucher_whitelist_enabled(env: Env) -> bool {
        admin::is_voucher_whitelist_enabled(env)
    }

    pub fn is_borrower_whitelisted(env: Env, borrower: Address) -> bool {
        admin::is_borrower_whitelisted(env, borrower)
    }

    pub fn is_borrower_whitelist_enabled(env: Env) -> bool {
        admin::is_borrower_whitelist_enabled(env)
    }

    pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
        loan::get_loan(env, borrower)
    }

    pub fn get_loan_by_id(env: Env, loan_id: u64) -> Option<LoanRecord> {
        loan::get_loan_by_id(env, loan_id)
    }

    pub fn get_loan_purpose(env: Env, loan_id: u64) -> Option<soroban_sdk::String> {
        loan::get_loan_purpose(env, loan_id)
    }

    pub fn get_loan_status(env: Env, loan_id: u64) -> LoanStatus {
        loan::get_loan_status(env, loan_id)
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

    /// Issue #461: Get the list of borrowers a voucher has backed.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    ///
    /// # Returns
    /// * `Vec<Address>` - Borrower addresses this voucher has backed (empty if none)
    pub fn get_voucher_history(env: Env, voucher: Address) -> Vec<Address> {
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

    pub fn get_config(env: Env) -> Config {
        admin::get_config(env)
    }

    pub fn get_admin_audit_log(env: Env) -> Vec<AdminAuditEntry> {
        admin::get_admin_audit_log(env)
    }

    pub fn set_admin_key_expiry(env: Env, admin_signers: Vec<Address>, admin: Address, expiry: u64) {
        admin::set_admin_key_expiry(env, admin_signers, admin, expiry)
    }

    pub fn get_admin_key_expiry(env: Env, admin: Address) -> u64 {
        admin::get_admin_key_expiry(env, admin)
    }

    pub fn queue_admin_action(
        env: Env,
        admin_signers: Vec<Address>,
        action: AdminTimelockAction,
        delay_secs: u64,
    ) -> Result<u64, ContractError> {
        admin::queue_admin_action(env, admin_signers, action, delay_secs)
    }

    pub fn execute_admin_action(env: Env, action_id: u64) -> Result<(), ContractError> {
        admin::execute_admin_action(env, action_id)
    }

    pub fn cancel_admin_action(
        env: Env,
        caller: Address,
        action_id: u64,
    ) -> Result<(), ContractError> {
        admin::cancel_admin_action(env, caller, action_id)
    }

    pub fn get_admin_timelock(env: Env, action_id: u64) -> Option<AdminTimelock> {
        admin::get_admin_timelock(env, action_id)
    }

    pub fn propose_governance_change(
        env: Env,
        proposer: Address,
        description: soroban_sdk::String,
        voting_period_secs: u64,
    ) -> Result<u64, ContractError> {
        governance::propose_governance_change(env, proposer, description, voting_period_secs)
    }

    pub fn vote_on_governance_change(
        env: Env,
        voter: Address,
        proposal_id: u64,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_on_governance_change(env, voter, proposal_id, approve)
    }

    pub fn execute_governance_change(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::execute_governance_change(env, proposal_id)
    }

    pub fn get_governance_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<GovernanceProposal> {
        governance::get_governance_proposal(env, proposal_id)
    }

    pub fn set_governance_token(
        env: Env,
        admin_signers: Vec<Address>,
        token: Address,
    ) -> Result<(), ContractError> {
        admin::set_governance_token(env, admin_signers, token)
    }

    pub fn get_governance_token(env: Env) -> Option<Address> {
        governance::get_governance_token(env)
    }

    pub fn set_voucher_stake_limit(
        env: Env,
        admin_signers: Vec<Address>,
        voucher: Address,
        borrower: Address,
        limit: i128,
    ) {
        admin::set_voucher_stake_limit(env, admin_signers, voucher, borrower, limit)
    }

    pub fn get_voucher_stake_limit(env: Env, voucher: Address, borrower: Address) -> Option<i128> {
        admin::get_voucher_stake_limit(env, voucher, borrower)
    }

    // ── Health Check ──────────────────────────────────────────────────────────

    pub fn health_check(env: Env) -> health::HealthStatus {
        health::health_check(&env)
    }

    // ── Upgrade Safety ────────────────────────────────────────────────────────

    pub fn validate_upgrade(env: Env, new_wasm_hash: soroban_sdk::BytesN<32>) -> Result<(), ContractError> {
        upgrade::validate_upgrade(&env, new_wasm_hash)
    }

    // ── #634: Liquidity Mining ────────────────────────────────────────────────

    pub fn claim_liquidity_mining_reward(env: Env, voucher: Address) -> Result<i128, ContractError> {
        liquidity_mining::claim_liquidity_mining_reward(env, voucher)
    }

    pub fn get_pending_mining_reward(env: Env, voucher: Address) -> i128 {
        liquidity_mining::get_pending_mining_reward(env, voucher)
    }

    // ── #635: Vouch Snapshot for Governance ──────────────────────────────────

    pub fn take_vouch_snapshot(env: Env, caller: Address) -> Result<u32, ContractError> {
        vouch_snapshot::take_vouch_snapshot(env, caller)
    }

    pub fn get_vouch_snapshot(env: Env, ledger_sequence: u32) -> Option<VouchSnapshotRecord> {
        vouch_snapshot::get_vouch_snapshot(env, ledger_sequence)
    }

    pub fn get_snapshot_stake(env: Env, ledger_sequence: u32, borrower: Address) -> i128 {
        vouch_snapshot::get_snapshot_stake(env, ledger_sequence, borrower)
    }

    // ── #636: Staking Derivatives ─────────────────────────────────────────────

    pub fn mint_staking_derivative(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        staking_derivatives::mint_staking_derivative(env, voucher, borrower)
    }

    pub fn transfer_staking_derivative(
        env: Env,
        from: Address,
        to: Address,
        original_voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        staking_derivatives::transfer_staking_derivative(env, from, to, original_voucher, borrower)
    }

    pub fn get_staking_derivative(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Option<StakingDerivativeRecord> {
        staking_derivatives::get_staking_derivative(env, voucher, borrower)
    }

    // ── #637: Fraud Detection ─────────────────────────────────────────────────

    pub fn calculate_fraud_score(env: Env, voucher: Address) -> u32 {
        fraud_detection::calculate_fraud_score(env, voucher)
    }

    pub fn get_fraud_score(env: Env, voucher: Address) -> u32 {
        fraud_detection::get_fraud_score(env, voucher)
    }

    pub fn is_high_fraud_risk(env: Env, voucher: Address) -> bool {
        fraud_detection::is_high_fraud_risk(env, voucher)
    }
}
