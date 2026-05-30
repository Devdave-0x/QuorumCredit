use crate::{
    admin,
    errors::ContractError,
    fraud_detection,
    governance,
    helpers::{self, require_valid_token, validate_admin_config},
    insurance,
    liquidity_mining,
    loan,
    reputation::ReputationNftExternalClient,
    staking_derivatives,
    types::*,
    vouch,
    vouch_snapshot,
};
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, Vec,
};

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

        validate_admin_config(&env, &admins, admin_threshold).expect("invalid admin config");
        require_valid_token(&env, &token).expect("invalid token");
        assert!(
            !env.storage().instance().has(&DataKey::Config),
            "already initialized"
        );

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
                min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
                prepayment_penalty_bps: 0,
                liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
                recovery_percentage: 0,
                redistribution_rule: RedistributionRule::Treasury,
                immunity_period_seconds: 0,
                insurance_premium_bps: 0,
                voting_period_seconds: crate::types::DEFAULT_VOTING_PERIOD_SECONDS,
                slash_cooldown_seconds: 0,
                emergency_pause_enabled: false,
                successor_admin: None,
            },
        );

        // Initialize API version (Issue #723)
        crate::versioning::initialize_api_version(&env);

        env.events().publish(
            (symbol_short!("contract"), symbol_short!("init")),
            (deployer, admins, admin_threshold, token),
        );
        Ok(())
    }

    // ── Vouch ─────────────────────────────────────────────────────────────────

    /// Vouch for a borrower by staking tokens.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher staking tokens
    /// * `borrower` - Address of the borrower being vouched for
    /// * `stake` - Amount of tokens to stake (must be positive)
    /// * `token` - Address of the token contract to stake
    ///
    /// # Panics
    /// * If voucher is the same as borrower
    /// * If stake is not greater than zero
    /// * If token is not allowed
    /// * If minimum stake requirement is not met
    /// * If borrower has an active loan
    /// * If duplicate vouch from same voucher for same borrower
    /// * If contract is paused
    pub fn vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        stake: i128,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::vouch(env, voucher, borrower, stake, token)
    }

    /// Vouch for multiple borrowers in a single transaction.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher staking tokens
    /// * `borrowers` - Vector of borrower addresses
    /// * `stakes` - Vector of stake amounts (must match borrowers length)
    /// * `token` - Address of the token contract to stake
    ///
    /// # Panics
    /// * If borrowers and stakes vectors have different lengths
    /// * If batch is empty
    /// * If any individual vouch fails (see `vouch` function)
    pub fn batch_vouch(
        env: Env,
        voucher: Address,
        borrowers: Vec<Address>,
        stakes: Vec<i128>,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::batch_vouch(env, voucher, borrowers, stakes, token)
    }

    /// Increase the stake for an existing vouch.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    /// * `borrower` - Address of the borrower
    /// * `additional` - Additional amount to stake (must be positive)
    ///
    /// # Panics
    /// * If vouch does not exist
    /// * If additional amount is not positive
    /// * If contract is paused
    pub fn increase_stake(
        env: Env,
        voucher: Address,
        borrower: Address,
        additional: i128,
    ) -> Result<(), ContractError> {
        vouch::increase_stake(env, voucher, borrower, additional)
    }

    /// Decrease the stake for an existing vouch.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    /// * `borrower` - Address of the borrower
    /// * `amount` - Amount to decrease (must be positive and not exceed current stake)
    ///
    /// # Panics
    /// * If vouch does not exist
    /// * If amount is not positive
    /// * If amount exceeds current stake
    /// * If borrower has an active loan
    /// * If contract is paused
    pub fn decrease_stake(
        env: Env,
        voucher: Address,
        borrower: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        vouch::decrease_stake(env, voucher, borrower, amount)
    }

    /// Withdraw a vouch completely and return the stake to the voucher.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    /// * `borrower` - Address of the borrower
    ///
    /// # Panics
    /// * If vouch does not exist
    /// * If borrower has an active loan
    /// * If contract is paused
    pub fn withdraw_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::withdraw_vouch(env, voucher, borrower)
    }

    /// Transfer a vouch from one address to another.
    ///
    /// # Arguments
    /// * `from` - Address of the current voucher
    /// * `to` - Address of the new voucher
    /// * `borrower` - Address of the borrower
    ///
    /// # Panics
    /// * If vouch does not exist for `from`
    /// * If borrower has an active loan
    /// * If contract is paused
    pub fn transfer_vouch(
        env: Env,
        from: Address,
        to: Address,
        borrower: Address,
    ) -> Result<(), ContractError> {
        vouch::transfer_vouch(env, from, to, borrower)
    }

    /// Issue #532: Delegate vouch management to another address.
    ///
    /// # Arguments
    /// * `voucher` - Address of the original voucher
    /// * `borrower` - Address of the borrower
    /// * `delegate` - Address to delegate vouch management to
    /// * `token` - Address of the token contract
    ///
    /// # Panics
    /// * If voucher is the same as delegate
    /// * If vouch does not exist
    /// * If contract is paused
    pub fn delegate_vouch(
        env: Env,
        voucher: Address,
        borrower: Address,
        delegate: Address,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::delegate_vouch(env, voucher, borrower, delegate, token)
    }

    /// Issue #532: Revoke delegation of a vouch.
    ///
    /// # Arguments
    /// * `voucher` - Address of the original voucher
    /// * `borrower` - Address of the borrower
    /// * `token` - Address of the token contract
    ///
    /// # Panics
    /// * If vouch does not exist
    /// * If contract is paused
    pub fn revoke_delegation(
        env: Env,
        voucher: Address,
        borrower: Address,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::revoke_delegation(env, voucher, borrower, token)
    }

    /// Issue #533: Set expiry timestamp for a vouch.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    /// * `borrower` - Address of the borrower
    /// * `expiry_timestamp` - Timestamp when the vouch expires
    /// * `token` - Address of the token contract
    ///
    /// # Panics
    /// * If expiry_timestamp is in the past
    /// * If vouch does not exist
    /// * If contract is paused
    pub fn set_vouch_expiry(
        env: Env,
        voucher: Address,
        borrower: Address,
        expiry_timestamp: u64,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::set_vouch_expiry(env, voucher, borrower, expiry_timestamp, token)
    }

    /// Issue #534: Get vouch modification history for auditing.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `voucher` - Address of the voucher
    /// * `token` - Address of the token contract
    ///
    /// # Returns
    /// * `Vec<VouchHistoryEntry>` - History of modifications
    pub fn get_vouch_history(
        env: Env,
        borrower: Address,
        voucher: Address,
        token: Address,
    ) -> Vec<VouchHistoryEntry> {
        vouch::get_vouch_history(env, borrower, voucher, token)
    }

    // ── Loan ──────────────────────────────────────────────────────────────────

    /// Register a referrer for a borrower. Must be called before `request_loan`.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `referrer` - Address of the referrer (cannot be the borrower)
    ///
    /// # Panics
    /// * If borrower is the same as referrer
    /// * If borrower has an active loan
    /// * If contract is paused
    pub fn register_referral(
        env: Env,
        borrower: Address,
        referrer: Address,
    ) -> Result<(), ContractError> {
        loan::register_referral(env, borrower, referrer)
    }

    /// Get the referrer for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `Option<Address>` - The referrer address if set, None otherwise
    pub fn get_referrer(env: Env, borrower: Address) -> Option<Address> {
        loan::get_referrer(env, borrower)
    }

    /// Set the referral bonus in basis points.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `bonus_bps` - Bonus in basis points (must not exceed 10000)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If bonus_bps exceeds 10000
    pub fn set_referral_bonus_bps(env: Env, admin_signers: Vec<Address>, bonus_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        assert!(bonus_bps <= 10_000, "bonus_bps must not exceed 10000");
        env.storage()
            .instance()
            .set(&DataKey::ReferralBonusBps, &bonus_bps);
    }

    /// Get the current referral bonus in basis points.
    ///
    /// # Returns
    /// * `u32` - The referral bonus in basis points
    pub fn get_referral_bonus_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ReferralBonusBps)
            .unwrap_or(crate::types::DEFAULT_REFERRAL_BONUS_BPS)
    }

    /// Request a loan from the protocol.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `amount` - Loan amount in stroops
    /// * `threshold` - Minimum total stake required from vouchers
    /// * `loan_purpose` - Description of the loan purpose
    /// * `token` - Address of the token contract for the loan
    ///
    /// # Panics
    /// * If borrower is blacklisted
    /// * If token is not allowed
    /// * If amount is below minimum loan amount
    /// * If threshold is not positive
    /// * If amount exceeds maximum loan amount
    /// * If borrower has an active loan
    /// * If total vouched stake is below threshold
    /// * If number of vouchers is below minimum
    /// * If any vouch is too recent
    /// * If loan amount exceeds maximum collateral ratio
    /// * If contract has insufficient balance
    /// * If contract is paused
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

    /// Repay a loan partially or fully.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `payment` - Payment amount in stroops (must be positive and not exceed outstanding balance)
    ///
    /// # Panics
    /// * If borrower does not have an active loan
    /// * If loan deadline has passed
    /// * If payment is not positive
    /// * If payment exceeds outstanding balance
    /// * If contract is paused
    pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
        loan::repay(env, borrower, payment)
    }

    /// Add a co-borrower to an active loan.
    ///
    /// # Arguments
    /// * `borrower` - Primary borrower address (must sign)
    /// * `co_borrower` - Address of the co-borrower to add
    ///
    /// # Errors
    /// * If borrower has no active loan
    /// * If co-borrower is the same as primary borrower
    /// * If co-borrower is already added
    /// * If contract is paused
    pub fn add_co_borrower(
        env: Env,
        borrower: Address,
        co_borrower: Address,
    ) -> Result<(), ContractError> {
        loan::add_co_borrower(env, borrower, co_borrower)
    }

    /// Refinance an existing loan with new terms.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower (must sign)
    /// * `new_amount` - New loan amount in stroops
    /// * `new_threshold` - New minimum stake threshold in stroops
    /// * `new_token` - Token contract address for the new loan
    ///
    /// # Errors
    /// * If borrower has no active loan
    /// * If new_amount or new_threshold is not positive
    /// * If new_amount is below minimum or exceeds maximum
    /// * If total stake is below threshold
    /// * If contract has insufficient balance
    /// * If token is not allowed
    /// * If contract is paused
    pub fn refinance_loan(
        env: Env,
        borrower: Address,
        new_amount: i128,
        new_threshold: i128,
        new_token: Address,
    ) -> Result<(), ContractError> {
        loan::refinance_loan(env, borrower, new_amount, new_threshold, new_token)
    }

    /// Deposit collateral for a borrower (required for high-risk borrowers).
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower (must sign)
    /// * `amount` - Collateral amount in stroops
    /// * `token` - Token contract address for collateral
    ///
    /// # Errors
    /// * If amount is not positive
    /// * If token is not allowed
    /// * If contract is paused
    pub fn deposit_collateral(
        env: Env,
        borrower: Address,
        amount: i128,
        token: Address,
    ) -> Result<(), ContractError> {
        loan::deposit_collateral(env, borrower, amount, token)
    }

    /// Get the collateral amount deposited by a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `i128` - Collateral amount in stroops
    pub fn get_borrower_collateral(env: Env, borrower: Address) -> i128 {
        loan::get_borrower_collateral(env, borrower)
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    /// Add a new admin to the protocol.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `new_admin` - Address of the new admin to add
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If new admin already exists
    /// * If new admin is a zero address
    pub fn add_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) {
        admin::add_admin(env, admin_signers, new_admin)
    }

    /// Remove an admin from the protocol.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `admin_to_remove` - Address of the admin to remove
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If admin to remove does not exist
    /// * If removal would leave fewer admins than threshold
    pub fn remove_admin(env: Env, admin_signers: Vec<Address>, admin_to_remove: Address) {
        admin::remove_admin(env, admin_signers, admin_to_remove)
    }

    /// Rotate an admin address to a new address.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `old_admin` - Address of the admin to replace
    /// * `new_admin` - Address of the new admin
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If old admin does not exist
    /// * If new admin already exists
    /// * If new admin is a zero address
    pub fn rotate_admin(
        env: Env,
        admin_signers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) {
        admin::rotate_admin(env, admin_signers, old_admin, new_admin)
    }

    /// Propose a new admin (two-step admin transfer).
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet current threshold)
    /// * `new_admin` - Address of the proposed new admin
    ///
    /// # Returns
    /// * `Result<(), ContractError>` - Success or error
    ///
    /// # Errors
    /// * `ContractError::ZeroAddress` - If new_admin is the zero address
    pub fn propose_admin(env: Env, admin_signers: Vec<Address>, new_admin: Address) -> Result<(), ContractError> {
        admin::propose_admin(env, admin_signers, new_admin)
    }

    /// Accept the proposed admin transfer.
    ///
    /// # Returns
    /// * `Result<(), ContractError>` - Success or error
    ///
    /// # Errors
    /// * `ContractError::UnauthorizedCaller` - If no pending admin is set or caller is not the pending admin
    pub fn accept_admin(env: Env) -> Result<(), ContractError> {
        admin::accept_admin(env)
    }

    /// Set the admin threshold (minimum number of admins required for approval).
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet current threshold)
    /// * `new_threshold` - New threshold value (must be > 0 and <= admin count)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If new_threshold is 0
    /// * If new_threshold exceeds admin count
    pub fn set_admin_threshold(env: Env, admin_signers: Vec<Address>, new_threshold: u32) {
        admin::set_admin_threshold(env, admin_signers, new_threshold)
    }

    /// Set the protocol fee in basis points.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `fee_bps` - Fee in basis points (must not exceed 10000)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If fee_bps exceeds 10000
    pub fn set_protocol_fee(env: Env, admin_signers: Vec<Address>, fee_bps: u32) {
        admin::set_protocol_fee(env, admin_signers, fee_bps)
    }

    /// Whitelist a voucher to allow them to vouch.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `voucher` - Address of the voucher to whitelist
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn whitelist_voucher(env: Env, admin_signers: Vec<Address>, voucher: Address) {
        admin::whitelist_voucher(env, admin_signers, voucher)
    }

    /// Set the fee treasury address where protocol fees are sent.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `treasury` - Address of the fee treasury
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_fee_treasury(env: Env, admin_signers: Vec<Address>, treasury: Address) {
        admin::set_fee_treasury(env, admin_signers, treasury)
    }

    /// Upgrade the contract to a new WASM hash.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `new_wasm_hash` - Hash of the new WASM code
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn upgrade(env: Env, admin_signers: Vec<Address>, new_wasm_hash: BytesN<32>) {
        admin::upgrade(env, admin_signers, new_wasm_hash)
    }

    /// Pause the contract (stops all operations except admin functions).
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn pause(env: Env, admin_signers: Vec<Address>) {
        admin::pause(env, admin_signers)
    }

    /// Unpause the contract (resumes all operations).
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn unpause(env: Env, admin_signers: Vec<Address>) {
        admin::unpause(env, admin_signers)
    }

    /// Pause the contract with a gradual thaw period for emergency withdrawals.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `thaw_duration` - Duration in seconds for the thaw period
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn pause_with_thaw(env: Env, admin_signers: Vec<Address>, thaw_duration: u64) {
        admin::pause_with_thaw(env, admin_signers, thaw_duration)
    }

    /// Blacklist a borrower (prevents them from requesting loans).
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `borrower` - Address of the borrower to blacklist
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn blacklist(env: Env, admin_signers: Vec<Address>, borrower: Address) {
        admin::blacklist(env, admin_signers, borrower)
    }

    /// Set the entire protocol configuration.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `config` - New configuration struct
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_config(env: Env, admin_signers: Vec<Address>, config: Config) {
        admin::set_config(env, admin_signers, config)
    }

    /// Update specific configuration parameters.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `yield_bps` - New yield in basis points (optional)
    /// * `slash_bps` - New slash in basis points (optional)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn update_config(
        env: Env,
        admin_signers: Vec<Address>,
        yield_bps: Option<i128>,
        slash_bps: Option<i128>,
    ) {
        admin::update_config(env, admin_signers, yield_bps, slash_bps)
    }

    /// Toggle dynamic slash threshold on/off.
    /// When enabled, slash penalties adjust based on protocol health.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `enabled` - Whether to enable dynamic slash threshold
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_dynamic_slash_threshold(
        env: Env,
        admin_signers: Vec<Address>,
        enabled: bool,
    ) {
        admin::set_dynamic_slash_threshold(env, admin_signers, enabled)
    }

    /// Get the current effective slash threshold (either static or dynamic).
    /// This function can be called by anyone to see what slash rate would be applied.
    ///
    /// # Returns
    /// * Current effective slash threshold in basis points
    pub fn get_effective_slash_threshold(env: Env) -> i128 {
        admin::get_effective_slash_threshold(env)
    }

    /// Toggle loan-size-based slash scaling on/off.
    /// When enabled, slash percentage scales linearly with loan size relative to
    /// total staked collateral. Small loans use `slash_bps`; large loans scale up
    /// to `loan_size_slash_max_bps`.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `enabled` - Whether to enable loan-size-based slash scaling
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_loan_size_slash_enabled(
        env: Env,
        admin_signers: Vec<Address>,
        enabled: bool,
    ) {
        admin::set_loan_size_slash_enabled(env, admin_signers, enabled)
    }

    /// Set the maximum slash rate for the largest loans when loan-size scaling is enabled.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `max_bps` - Maximum slash rate in basis points (must be >= slash_bps, <= 10_000)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If max_bps < slash_bps or max_bps > 10_000
    pub fn set_loan_size_slash_max_bps(
        env: Env,
        admin_signers: Vec<Address>,
        max_bps: i128,
    ) {
        admin::set_loan_size_slash_max_bps(env, admin_signers, max_bps)
    }

    /// Set the reputation NFT contract address.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `nft_contract` - Address of the reputation NFT contract
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_reputation_nft(env: Env, admin_signers: Vec<Address>, nft_contract: Address) {
        admin::set_reputation_nft(env, admin_signers, nft_contract)
    }

    /// Set the minimum stake amount per vouch.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `amount` - Minimum stake amount in stroops
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_min_stake(env: Env, admin_signers: Vec<Address>, amount: i128) {
        admin::set_min_stake(env, admin_signers, amount)
    }

    /// Set the maximum loan amount.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `amount` - Maximum loan amount in stroops (0 = no cap)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_max_loan_amount(env: Env, admin_signers: Vec<Address>, amount: i128) {
        admin::set_max_loan_amount(env, admin_signers, amount)
    }

    /// Set the minimum number of vouchers required for a loan.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `count` - Minimum number of vouchers (0 = no minimum)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_min_vouchers(env: Env, admin_signers: Vec<Address>, count: u32) {
        admin::set_min_vouchers(env, admin_signers, count)
    }

    /// Set the maximum loan-to-stake ratio.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `ratio` - Maximum ratio in basis points (e.g., 15000 = 150%)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_max_loan_to_stake_ratio(env: Env, admin_signers: Vec<Address>, ratio: u32) {
        admin::set_max_loan_to_stake_ratio(env, admin_signers, ratio)
    }

    /// Add a token to the allowed tokens list.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `token` - Address of the token to add
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_max_vouchers_per_borrower(env: Env, admin_signers: Vec<Address>, max_vouchers: u32) {
        admin::set_max_vouchers_per_borrower(env, admin_signers, max_vouchers)
    }

    pub fn add_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) -> Result<(), ContractError> {
        admin::add_allowed_token(env, admin_signers, token)
    }

    /// Remove a token from the allowed tokens list.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `token` - Address of the token to remove
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn remove_allowed_token(env: Env, admin_signers: Vec<Address>, token: Address) {
        admin::remove_allowed_token(env, admin_signers, token)
    }

    /// Set the grace period after loan deadline before slashing is allowed.
    pub fn set_grace_period(env: Env, admin_signers: Vec<Address>, period: u64) {
        admin::set_grace_period(env, admin_signers, period)
    }

    /// Enable or disable the voucher whitelist.
    pub fn set_whitelist_enabled(env: Env, admin_signers: Vec<Address>, enabled: bool) {
        admin::set_whitelist_enabled(env, admin_signers, enabled)
    }

    /// Withdraw funds from the slash treasury to a recipient address.
    /// Admin-gated. Emits an admin/slshwdraw event on success.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `recipient` - Address to receive the withdrawn funds
    /// * `amount` - Amount to withdraw in stroops (must be > 0)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If amount is not greater than zero
    /// * If slash treasury balance is insufficient
    pub fn withdraw_slash_treasury(
        env: Env,
        admin_signers: Vec<Address>,
        recipient: Address,
        amount: i128,
    ) {
        admin::withdraw_slash_treasury(env, admin_signers, recipient, amount)
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Get the current list of admin addresses.
    ///
    /// # Returns
    /// * `Vec<Address>` - The list of admin addresses
    pub fn get_admins(env: Env) -> Vec<Address> {
        helpers::get_admins(&env)
    }

    /// Get the current protocol configuration.
    ///
    /// # Returns
    /// * `Config` - The current configuration struct
    pub fn get_config(env: Env) -> Config {
        helpers::config(&env)
    }

    /// Get the accumulated protocol fees in the fee treasury.
    ///
    /// # Returns
    /// * `i128` - The balance of the fee treasury address in stroops
    pub fn get_fee_treasury(env: Env) -> i128 {
        let fee_treasury: Option<Address> = env
            .storage()
            .instance()
            .get(&DataKey::FeeTreasury);
        match fee_treasury {
            Some(address) => {
                let token_client = helpers::primary_token(&env);
                token_client.balance(&address)
            }
            None => 0,
        }
    }

    // ── Governance ────────────────────────────────────────────────────────────

    /// Vote on a slash proposal for a borrower.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher voting
    /// * `borrower` - Address of the borrower being voted on
    /// * `approve` - True to approve slash, false to reject
    ///
    /// # Panics
    /// * If voucher has not vouched for borrower
    /// * If voucher has already voted
    /// * If contract is paused
    pub fn vote_slash(
        env: Env,
        voucher: Address,
        borrower: Address,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_slash(env, voucher, borrower, approve)
    }

    /// Set the slash vote quorum in basis points.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `quorum_bps` - Quorum in basis points (e.g., 5000 = 50%)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    pub fn set_slash_vote_quorum(env: Env, admin_signers: Vec<Address>, quorum_bps: u32) {
        helpers::require_admin_approval(&env, &admin_signers);
        governance::set_slash_vote_quorum(&env, quorum_bps);
    }

    /// Get the current slash vote quorum in basis points.
    ///
    /// # Returns
    /// * `u32` - The quorum in basis points
    pub fn get_slash_vote_quorum(env: Env) -> u32 {
        governance::get_slash_vote_quorum(env)
    }

    /// Set the prepayment penalty in basis points.
    ///
    /// # Arguments
    /// * `admin_signers` - Vector of admin addresses (must meet threshold)
    /// * `penalty_bps` - Penalty in basis points (e.g., 100 = 1%)
    ///
    /// # Panics
    /// * If admin approval is insufficient
    /// * If penalty_bps exceeds 10000
    pub fn set_prepayment_penalty_bps(env: Env, admin_signers: Vec<Address>, penalty_bps: u32) {
        admin::set_prepayment_penalty_bps(env, admin_signers, penalty_bps)
    }

    /// Get the current prepayment penalty in basis points.
    ///
    /// # Returns
    /// * `u32` - The prepayment penalty in basis points
    pub fn get_prepayment_penalty_bps(env: Env) -> u32 {
        admin::get_prepayment_penalty_bps(env)
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    /// Check if the contract has been initialized.
    ///
    /// # Returns
    /// * `bool` - True if initialized, false otherwise
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    /// Get the primary token address.
    ///
    /// # Returns
    /// * `Address` - The token address
    pub fn get_token(env: Env) -> Address {
        helpers::config(&env).token
    }

    /// Get the list of admin addresses.
    ///
    /// # Returns
    /// * `Vec<Address>` - Vector of admin addresses
    pub fn get_admin_threshold(env: Env) -> u32 {
        admin::get_admin_threshold(env)
    }

    /// Get the slash treasury balance.
    ///
    /// # Returns
    /// * `i128` - The slash treasury balance in stroops
    pub fn get_slash_treasury_balance(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::SlashTreasury)
            .unwrap_or(0)
    }

    /// Check if the contract is paused.
    ///
    /// # Returns
    /// * `bool` - True if paused, false otherwise
    pub fn get_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Get the loan status for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `LoanStatus` - The current loan status
    pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
        loan::loan_status(env, borrower)
    }

    /// Check if a vouch exists for a borrower.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `bool` - True if vouch exists, false otherwise
    pub fn vouch_exists(env: Env, voucher: Address, borrower: Address) -> bool {
        vouch::vouch_exists(env, voucher, borrower)
    }

    /// Check if a voucher is whitelisted.
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    ///
    /// # Returns
    /// * `bool` - True if whitelisted, false otherwise
    pub fn is_whitelisted(env: Env, voucher: Address) -> bool {
        admin::is_whitelisted(env, voucher)
    }

    /// Get the loan record for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `Option<LoanRecord>` - The loan record if exists, None otherwise
    pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
        loan::get_loan(env, borrower)
    }

    /// Get a loan record by ID.
    ///
    /// # Arguments
    /// * `loan_id` - The loan ID
    ///
    /// # Returns
    /// * `Option<LoanRecord>` - The loan record if exists, None otherwise
    pub fn get_loan_by_id(env: Env, loan_id: u64) -> Option<LoanRecord> {
        loan::get_loan_by_id(env, loan_id)
    }

    /// Get all vouches for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `Option<Vec<VouchRecord>>` - Vector of vouch records if any exist, None otherwise
    pub fn get_vouches(env: Env, borrower: Address) -> Option<Vec<VouchRecord>> {
        env.storage().persistent().get(&DataKey::Vouches(borrower))
    }

    /// Check if a borrower is eligible for a loan.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `threshold` - Minimum total stake required
    /// * `token_addr` - Token address to filter vouches by
    ///
    /// # Returns
    /// * `bool` - True if eligible, false otherwise
    pub fn is_eligible(env: Env, borrower: Address, threshold: i128, token_addr: Address) -> bool {
        loan::is_eligible(env, borrower, threshold, token_addr)
    }

    /// Get the contract's token balance.
    ///
    /// # Returns
    /// * `i128` - The contract balance in stroops
    pub fn get_contract_balance(env: Env) -> i128 {
        helpers::primary_token(&env).balance(&env.current_contract_address())
    }

    /// Get the voucher history (list of borrowers vouched for).
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    ///
    /// # Returns
    /// * `Vec<Address>` - Vector of borrower addresses
    pub fn voucher_history(env: Env, voucher: Address) -> Vec<Address> {
        vouch::voucher_history(env, voucher)
    }

    /// Get cumulative reputation statistics for a voucher (issue #602).
    ///
    /// # Arguments
    /// * `voucher` - Address of the voucher
    ///
    /// # Returns
    /// * `VoucherStats` - Struct with successful_vouches, total_vouches_slashed,
    ///   total_yield_earned, and total_slashed. Returns zeroed stats if no history.
    pub fn get_voucher_stats(env: Env, voucher: Address) -> VoucherStats {
        vouch::get_voucher_stats(env, voucher)
    }

    /// Get the reputation score for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `u32` - The reputation score
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

    /// Get the total amount vouched for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `Result<i128, ContractError>` - Total vouched amount or error if overflow
    pub fn total_vouched(env: Env, borrower: Address) -> Result<i128, ContractError> {
        vouch::total_vouched(env, borrower)
    }

    /// Get the repayment count for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `u32` - The number of successful repayments
    pub fn repayment_count(env: Env, borrower: Address) -> u32 {
        loan::repayment_count(env, borrower)
    }

    /// Get the loan count for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `u32` - The total number of historical loans
    pub fn loan_count(env: Env, borrower: Address) -> u32 {
        loan::loan_count(env, borrower)
    }

    /// Get the default count for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `u32` - The total number of defaults
    pub fn default_count(env: Env, borrower: Address) -> u32 {
        loan::default_count(env, borrower)
    }

    /// Get payment history for a loan. (#598)
    pub fn get_payment_history(
        env: Env,
        loan_id: u64,
    ) -> Vec<crate::types::PaymentRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::PaymentHistory(loan_id))
            .unwrap_or(Vec::new(&env))
    }

    // ── Pagination ────────────────────────────────────────────────────────────

    /// Get paginated loans for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `limit` - Maximum results (default 10, max 100)
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    /// * `PaginatedLoans` - Paginated loan records
    pub fn get_loans_paginated(
        env: Env,
        borrower: Address,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> crate::types::PaginatedLoans {
        let params = crate::pagination::normalize_pagination(limit, offset);
        let loans = Vec::new(&env);
        let total = 0u32;
        crate::pagination::paginate_loans(&env, loans, total, params.limit, params.offset)
    }

    /// Get paginated vouches for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    /// * `limit` - Maximum results (default 10, max 100)
    /// * `offset` - Pagination offset
    ///
    /// # Returns
    /// * `PaginatedVouches` - Paginated vouch records
    pub fn get_vouches_paginated(
        env: Env,
        borrower: Address,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> crate::types::PaginatedVouches {
        let params = crate::pagination::normalize_pagination(limit, offset);
        if let Some(vouches) = env.storage().persistent().get::<_, Vec<VouchRecord>>(&DataKey::Vouches(borrower)) {
            let total = vouches.len();
            crate::pagination::paginate_vouches(&env, vouches, total, params.limit, params.offset)
        } else {
            crate::types::PaginatedVouches {
                vouches: Vec::new(&env),
                total: 0,
                limit: params.limit,
                offset: params.offset,
            }
        }
    }

    // ── Signature Verification ────────────────────────────────────────────────

    /// Verify that a caller has signed the transaction.
    /// Used to ensure the caller owns the address they claim.
    ///
    /// # Arguments
    /// * `caller` - Address claiming to make the request
    ///
    /// # Returns
    /// * `Result<(), ContractError>` - Ok if signed, Err otherwise
    pub fn verify_signature(env: Env, caller: Address) -> Result<(), ContractError> {
        crate::signature::verify_caller_signature(&env, &caller)
    }

    // ── Pause with Thaw ───────────────────────────────────────────────────────

    /// Check if the contract is in thaw period (gradual recovery after pause).
    ///
    /// # Returns
    /// * `bool` - True if in thaw period, false otherwise
    pub fn is_in_thaw_period(env: Env) -> bool {
        admin::is_in_thaw_period(&env)
    }

    /// Get the protocol fee in basis points.
    ///
    /// # Returns
    /// * `u32` - The protocol fee in basis points
    pub fn get_protocol_fee(env: Env) -> u32 {
        admin::get_protocol_fee(env)
    }

    /// Check if a borrower is blacklisted.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `bool` - True if blacklisted, false otherwise
    pub fn is_blacklisted(env: Env, borrower: Address) -> bool {
        admin::is_blacklisted(env, borrower)
    }

    /// Get the minimum stake amount per vouch.
    ///
    /// # Returns
    /// * `i128` - The minimum stake amount in stroops
    pub fn get_min_stake(env: Env) -> i128 {
        admin::get_min_stake(env)
    }

    /// Get the maximum loan amount.
    ///
    /// # Returns
    /// * `i128` - The maximum loan amount in stroops (0 = no cap)
    pub fn get_max_loan_amount(env: Env) -> i128 {
        admin::get_max_loan_amount(env)
    }

    /// Get the minimum number of vouchers required for a loan.
    ///
    /// # Returns
    /// * `u32` - The minimum number of vouchers (0 = no minimum)
    pub fn get_min_vouchers(env: Env) -> u32 {
        admin::get_min_vouchers(env)
    }

    /// Get the maximum loan-to-stake ratio.
    ///
    /// # Returns
    /// * `u32` - The maximum ratio in basis points
    pub fn get_max_loan_to_stake_ratio(env: Env) -> u32 {
        admin::get_max_loan_to_stake_ratio(env)
    }

    /// Get the maximum number of vouchers per borrower.
    ///
    /// # Returns
    /// * `u32` - The maximum number of vouchers per borrower
    pub fn get_max_vouchers_per_borrower(env: Env) -> u32 {
        admin::get_max_vouchers_per_borrower(env)
    }

    /// Issue 109: Propose a slash action with a confirmation window (timelock delay).
    pub fn propose_slash(
        env: Env,
        proposer: Address,
        borrower: Address,
        delay_secs: u64,
    ) -> Result<u64, ContractError> {
        governance::propose_slash(env, proposer, borrower, delay_secs)
    }

    /// Issue 109: Execute a previously proposed slash after the delay has passed.
    pub fn execute_slash_proposal(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::execute_slash_proposal(env, proposal_id)
    }

    /// Cancel a pending slash proposal (only proposer can cancel).
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the proposer)
    /// * `proposal_id` - The proposal ID to cancel
    ///
    /// # Returns
    /// * `Result<(), ContractError>` - Success or error
    ///
    /// # Panics
    /// * If proposal does not exist
    /// * If caller is not the proposer
    /// * If proposal has already been executed or cancelled
    pub fn cancel_slash_proposal(
        env: Env,
        caller: Address,
        proposal_id: u64,
    ) -> Result<(), ContractError> {
        governance::cancel_slash_proposal(env, caller, proposal_id)
    }

    /// Get a timelock proposal details.
    ///
    /// # Arguments
    /// * `proposal_id` - The proposal ID
    ///
    /// # Returns
    /// * `Option<TimelockProposal>` - The proposal details if exists, None otherwise
    pub fn get_timelock_proposal(env: Env, proposal_id: u64) -> Option<TimelockProposal> {
        governance::get_timelock_proposal(env, proposal_id)
    }

    /// Get the slash vote record for a borrower.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower
    ///
    /// # Returns
    /// * `Option<SlashVoteRecord>` - The slash vote record if exists, None otherwise
    pub fn get_slash_vote(env: Env, borrower: Address) -> Option<SlashVoteRecord> {
        governance::get_slash_vote(env, borrower)
    }

    /// Execute a slash vote if quorum has been met.
    ///
    /// # Arguments
    /// * `borrower` - Address of the borrower whose slash vote to execute
    ///
    /// # Returns
    /// * `Result<(), ContractError>` - Success or error
    ///
    /// # Errors
    /// * `ContractError::SlashVoteNotFound` - If no slash vote exists for the borrower
    /// * `ContractError::SlashAlreadyExecuted` - If the slash has already been executed
    /// * `ContractError::QuorumNotMet` - If the approval stake does not meet the quorum threshold
    pub fn execute_slash_vote(env: Env, borrower: Address) -> Result<(), ContractError> {
        governance::execute_slash_vote(env, borrower)
    }

    /// Emit `repayment_reminder` events for all active loans whose deadline is within 7 days.
    ///
    /// Off-chain systems can call this to trigger reminder events for borrowers approaching
    /// their repayment deadline.
    pub fn emit_repayment_reminders(env: Env) {
        loan::emit_repayment_reminders(env)
    }

    /// Mint a reputation NFT for a borrower who has repaid at least one loan.
    ///
    /// # Errors
    /// * `NoActiveLoan` — borrower has no successful repayments or no NFT contract configured
    pub fn mint_reputation_nft(env: Env, borrower: Address) -> Result<(), ContractError> {
        loan::mint_reputation_nft(env, borrower)
    }

    /// Calculate the dynamic yield in basis points for a borrower.
    ///
    /// Formula: `base_yield_bps + (credit_score / 100) - (default_count * 50)`, floored at 0.
    pub fn calculate_dynamic_yield(env: Env, borrower: Address) -> i128 {
        loan::calculate_dynamic_yield(&env, &borrower)
    }

    // ── Insurance Pool ────────────────────────────────────────────────────────

    /// Contribute tokens to the insurance pool.
    pub fn contribute_to_insurance(
        env: Env,
        contributor: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        insurance::contribute_to_insurance(env, contributor, amount)
    }

    /// Claim an insurance payout for a defaulted loan.
    pub fn claim_insurance(
        env: Env,
        voucher: Address,
        loan_id: u64,
    ) -> Result<(), ContractError> {
        insurance::claim_insurance(env, voucher, loan_id)
    }

    /// Get the current insurance pool balance in stroops.
    pub fn get_insurance_pool_balance(env: Env) -> i128 {
        insurance::get_insurance_pool_balance(env)
    }

    // ── Issue #535: Minimum Vouch Age ────────────────────────────────────────

    /// Request vouch withdrawal with timelock (Issue #537).
    pub fn request_vouch_withdrawal(
        env: Env,
        voucher: Address,
        borrower: Address,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::request_vouch_withdrawal(env, voucher, borrower, token)
    }

    /// Execute vouch withdrawal after timelock expires (Issue #537).
    pub fn execute_vouch_withdrawal(
        env: Env,
        voucher: Address,
        borrower: Address,
        token: Address,
    ) -> Result<(), ContractError> {
        vouch::execute_vouch_withdrawal(env, voucher, borrower, token)
    }

    /// Get slash audit record for a borrower (Issue #536).
    pub fn get_slash_record(env: Env, slash_id: u64) -> Option<crate::types::SlashRecord> {
        governance::get_slash_record(env, slash_id)
    }

    pub fn get_slash_audit(env: Env, borrower: Address) -> Option<crate::types::SlashRecord> {
        governance::get_slash_record_for_borrower(env, borrower)
    }

    /// Admin-only: reverse a slash and restore slashed funds to the borrower.
    pub fn reverse_slash(
        env: Env,
        admin_signers: Vec<Address>,
        slash_id: u64,
        reason: soroban_sdk::String,
    ) -> Result<(), ContractError> {
        governance::reverse_slash(env, admin_signers, slash_id, reason)
    }

    /// Repay loan with partial payment support (Issue #538).
    pub fn repay_partial(
        env: Env,
        borrower: Address,
        payment: i128,
        token: Address,
    ) -> Result<(), ContractError> {
        loan::repay_partial(env, borrower, payment, token)
    }

    // ── API Versioning (Issue #723) ───────────────────────────────────────────

    /// Get the current API version of the contract.
    ///
    /// Returns the semantic version (major.minor.patch) of the contract's API.
    /// Clients can use this to determine compatibility and handle version-specific behavior.
    pub fn get_api_version(env: Env) -> ApiVersion {
        crate::versioning::get_api_version(&env)
    }

    /// Check if the contract supports a specific API version.
    ///
    /// # Arguments
    /// * `major` - Major version number
    /// * `minor` - Minor version number
    /// * `patch` - Patch version number
    ///
    /// Returns true if the requested version is compatible with the current version.
    pub fn is_version_compatible(env: Env, major: u32, minor: u32, patch: u32) -> bool {
        let current = crate::versioning::get_api_version(&env);
        crate::versioning::is_version_compatible(
            (major, minor, patch),
            (current.major, current.minor, current.patch),
        )
    }

    // ── API Caching (Issue #724) ──────────────────────────────────────────────

    /// Get a loan record with caching support.
    ///
    /// This function returns a cached loan record if available and valid,
    /// otherwise fetches from storage and caches the result.
    pub fn get_loan_cached(env: Env, borrower: Address) -> Option<LoanRecord> {
        if let Some(loan_id) = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::ActiveLoan(borrower.clone()))
        {
            // Try to get from cache first
            if let Some(cached) = crate::cache::get_cached_loan(&env, loan_id) {
                return Some(cached);
            }

            // Fall back to storage
            if let Some(loan) = env
                .storage()
                .instance()
                .get::<DataKey, LoanRecord>(&DataKey::Loan(loan_id))
            {
                crate::cache::set_cached_loan(&env, loan_id, loan.clone());
                return Some(loan);
            }
        }
        None
    }

    /// Get vouches for a borrower with caching support.
    ///
    /// This function returns cached vouches if available and valid,
    /// otherwise fetches from storage and caches the result.
    pub fn get_vouches_cached(env: Env, borrower: Address) -> Option<Vec<VouchRecord>> {
        // Try to get from cache first
        if let Some(cached) = crate::cache::get_cached_vouches(&env, &borrower) {
            return Some(cached);
        }

        // Fall back to storage
        if let Some(vouches) = env
            .storage()
            .instance()
            .get::<DataKey, Vec<VouchRecord>>(&DataKey::Vouches(borrower.clone()))
        {
            crate::cache::set_cached_vouches(&env, &borrower, vouches.clone());
            return Some(vouches);
        }
        None
    }

    /// Get config with caching support.
    ///
    /// This function returns cached config if available and valid,
    /// otherwise fetches from storage and caches the result.
    pub fn get_config_cached(env: Env) -> Option<Config> {
        // Try to get from cache first
        if let Some(cached) = crate::cache::get_cached_config(&env) {
            return Some(cached);
        }

        // Fall back to storage
        if let Some(config) = env
            .storage()
            .instance()
            .get::<DataKey, Config>(&DataKey::Config)
        {
            crate::cache::set_cached_config(&env, config.clone());
            return Some(config);
        }
        None
    }

    /// Clear all caches (admin only).
    ///
    /// This function invalidates all cached records. Useful after configuration changes.
    pub fn clear_all_caches(env: Env, admin_signers: Vec<Address>) -> Result<(), ContractError> {
        admin::require_admin_auth(&env, &admin_signers)?;
        crate::cache::invalidate_config_cache(&env);
        Ok(())
    }

    // ── Error Standardization (Issue #725) ────────────────────────────────────

    /// Get a standardized error response for a given error code.
    ///
    /// This function returns a structured error response that includes:
    /// - Numeric error code
    /// - Human-readable message
    /// - Optional additional details
    /// - Timestamp of when the error occurred
    pub fn get_error_response(env: Env, error_code: u32) -> Option<ErrorResponse> {
        // Map error codes to ContractError variants
        let error = match error_code {
            1 => ContractError::InsufficientFunds,
            2 => ContractError::ActiveLoanExists,
            3 => ContractError::StakeOverflow,
            4 => ContractError::ZeroAddress,
            5 => ContractError::DuplicateVouch,
            6 => ContractError::NoActiveLoan,
            7 => ContractError::ContractPaused,
            8 => ContractError::LoanPastDeadline,
            13 => ContractError::MinStakeNotMet,
            14 => ContractError::LoanExceedsMaxAmount,
            15 => ContractError::InsufficientVouchers,
            16 => ContractError::UnauthorizedCaller,
            17 => ContractError::InvalidAmount,
            18 => ContractError::InvalidStateTransition,
            19 => ContractError::AlreadyInitialized,
            20 => ContractError::VouchTooRecent,
            24 => ContractError::Blacklisted,
            30 => ContractError::InvalidToken,
            31 => ContractError::AlreadyVoted,
            32 => ContractError::SlashVoteNotFound,
            33 => ContractError::SlashAlreadyExecuted,
            34 => ContractError::LoanBelowMinAmount,
            35 => ContractError::QuorumNotMet,
            _ => return None,
        };

        Some(crate::error_response::error_to_response(&env, error))
    }

    // ── Issue #687: Governance-based admin removal ────────────────────────────

    /// Propose removing a compromised admin via governance vote.
    ///
    /// Any governance participant (active voucher or admin) may call this.
    /// Requires `Config.removal_vote_threshold > 0`.
    pub fn propose_admin_removal(
        env: Env,
        proposer: Address,
        admin_to_remove: Address,
    ) -> Result<u64, ContractError> {
        governance::propose_admin_removal(env, proposer, admin_to_remove)
    }

    /// Vote on an admin removal proposal.
    ///
    /// Any governance participant may vote once per proposal.
    pub fn vote_admin_removal(
        env: Env,
        voter: Address,
        proposal_id: u64,
        approve: bool,
    ) -> Result<(), ContractError> {
        governance::vote_admin_removal(env, voter, proposal_id, approve)
    }

    /// Finalize an admin removal proposal once the vote threshold is met.
    ///
    /// Removes the targeted admin from `Config.admins` on success.
    pub fn finalize_admin_removal(env: Env, proposal_id: u64) -> Result<(), ContractError> {
        governance::finalize_admin_removal(env, proposal_id)
    }

    /// Return an admin removal proposal by ID.
    pub fn get_admin_removal_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Option<AdminRemovalProposal> {
        governance::get_admin_removal_proposal(env, proposal_id)
    }

    /// Set the minimum number of governance votes needed to remove an admin.
    ///
    /// 0 disables governance removal (admins can only be removed via multi-sig).
    pub fn set_removal_vote_threshold(
        env: Env,
        admin_signers: Vec<Address>,
        threshold: u32,
    ) {
        admin::set_removal_vote_threshold(env, admin_signers, threshold)
    }

    // ── Issue #686: Admin compensation ───────────────────────────────────────

    /// Set the admin compensation rate in basis points.
    ///
    /// Controls what fraction of the compensation pool each admin earns per claim.
    /// 0 disables admin compensation.
    pub fn set_admin_compensation_bps(
        env: Env,
        admin_signers: Vec<Address>,
        compensation_bps: u32,
    ) {
        admin::set_admin_compensation_bps(env, admin_signers, compensation_bps)
    }

    /// Add funds to the admin compensation pool.
    ///
    /// The `funder` transfers `amount` tokens to the contract's compensation pool.
    /// These funds are later claimed by admins via `claim_admin_compensation`.
    pub fn fund_admin_compensation(
        env: Env,
        funder: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        admin::fund_admin_compensation(env, funder, amount)
    }

    /// Claim admin compensation.
    ///
    /// The calling admin receives their pro-rata share of
    /// `pool * admin_compensation_bps / 10_000 / num_admins`.
    /// Limited to once per 24 hours per admin.
    /// Returns the amount claimed, in stroops.
    pub fn claim_admin_compensation(env: Env, admin: Address) -> Result<i128, ContractError> {
        admin::claim_admin_compensation(env, admin)
    }

    /// Return the current admin compensation rate in basis points.
    pub fn get_admin_compensation_bps(env: Env) -> u32 {
        admin::get_admin_compensation_bps(env)
    }

    /// Return the current balance of the admin compensation pool, in stroops.
    pub fn get_admin_compensation_pool(env: Env) -> i128 {
        admin::get_admin_compensation_pool(env)
    }
}
