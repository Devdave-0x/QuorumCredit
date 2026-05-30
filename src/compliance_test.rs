#[cfg(test)]
mod compliance_tests {
    use crate::errors::ContractError;
    use crate::types::{
        BPS_DENOMINATOR, DEFAULT_MIN_LOAN_AMOUNT, DEFAULT_MIN_YIELD_STAKE, DEFAULT_SLASH_BPS,
        DEFAULT_YIELD_BPS,
    };
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn _setup_contract(env: &Env) -> (QuorumCreditContractClient, Address, Address) {
        let deployer = Address::generate(env);
        let admin = Address::generate(env);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(env, &contract_id);
        let admins = Vec::from_array(env, [admin.clone()]);
        client.initialize(&deployer, &admins, &1u32, &token);
        (client, admin, token)
    }

    // ── Error code uniqueness ─────────────────────────────────────────────────

    /// Verify all ContractError discriminants are unique (no duplicate codes).
    #[test]
    fn test_error_codes_are_unique() {
        let codes: &[u32] = &[
            ContractError::InsufficientFunds as u32,
            ContractError::ActiveLoanExists as u32,
            ContractError::StakeOverflow as u32,
            ContractError::ZeroAddress as u32,
            ContractError::DuplicateVouch as u32,
            ContractError::NoActiveLoan as u32,
            ContractError::ContractPaused as u32,
            ContractError::LoanPastDeadline as u32,
            ContractError::MinStakeNotMet as u32,
            ContractError::LoanExceedsMaxAmount as u32,
            ContractError::InsufficientVouchers as u32,
            ContractError::UnauthorizedCaller as u32,
            ContractError::InvalidAmount as u32,
            ContractError::InvalidStateTransition as u32,
            ContractError::AlreadyInitialized as u32,
            ContractError::VouchTooRecent as u32,
            ContractError::Blacklisted as u32,
            ContractError::TimelockNotFound as u32,
            ContractError::TimelockNotReady as u32,
            ContractError::TimelockExpired as u32,
            ContractError::NoVouchesForBorrower as u32,
            ContractError::VoucherNotFound as u32,
            ContractError::InvalidToken as u32,
            ContractError::AlreadyVoted as u32,
            ContractError::SlashVoteNotFound as u32,
            ContractError::SlashAlreadyExecuted as u32,
            ContractError::SelfVouchNotAllowed as u32,
            ContractError::DuplicateToken as u32,
            ContractError::InsufficientVoucherBalance as u32,
            ContractError::MaxVouchersPerBorrowerExceeded as u32,
            ContractError::LoanBelowMinAmount as u32,
            ContractError::QuorumNotMet as u32,
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len(), "duplicate error codes detected");
    }

    // ── Key invariants ────────────────────────────────────────────────────────

    /// slash_bps must be in [0, 10000].
    #[test]
    fn test_slash_bps_within_bounds() {
        assert!(DEFAULT_SLASH_BPS >= 0);
        assert!(DEFAULT_SLASH_BPS <= BPS_DENOMINATOR);
    }

    /// yield_bps must be in [0, 10000].
    #[test]
    fn test_yield_bps_within_bounds() {
        assert!(DEFAULT_YIELD_BPS >= 0);
        assert!(DEFAULT_YIELD_BPS <= BPS_DENOMINATOR);
    }

    /// min_stake constant must be positive.
    #[test]
    fn test_min_yield_stake_positive() {
        assert!(DEFAULT_MIN_YIELD_STAKE > 0);
    }

    /// min_loan_amount constant must be positive.
    #[test]
    fn test_min_loan_amount_positive() {
        assert!(DEFAULT_MIN_LOAN_AMOUNT > 0);
    }

    // ── Auth compliance ───────────────────────────────────────────────────────

    /// initialize requires deployer auth — calling without auth must panic.
    #[test]
    #[should_panic]
    fn test_initialize_requires_deployer_auth() {
        let env = Env::default();
        // No mock_all_auths — auth is enforced
        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admins = Vec::from_array(&env, [admin]);
        client.initialize(&deployer, &admins, &1u32, &token);
    }

    /// vouch requires voucher auth — calling without auth must panic.
    #[test]
    #[should_panic]
    fn test_vouch_requires_voucher_auth() {
        let env = Env::default();
        // No mock_all_auths — auth is enforced
        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        let admins = Vec::from_array(&env, [admin]);
        client.initialize(&deployer, &admins, &1u32, &token);
        // Clear mocked auths so subsequent calls require real auth
        env.set_auths(&[]);
        let voucher = Address::generate(&env);
        let borrower = Address::generate(&env);
        // No auth — must panic
        client.vouch(&voucher, &borrower, &1_000_000i128, &token);
    }

    /// request_loan requires borrower auth — calling without auth must panic.
    #[test]
    #[should_panic]
    fn test_request_loan_requires_borrower_auth() {
        let env = Env::default();
        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        env.mock_all_auths();
        let admins = Vec::from_array(&env, [admin]);
        client.initialize(&deployer, &admins, &1u32, &token);
        // Clear mocked auths so subsequent calls require real auth
        env.set_auths(&[]);
        let borrower = Address::generate(&env);
        // No auth — must panic
        client.request_loan(&borrower, &1_000_000i128, &1_000_000i128);
    }

    // ── SEP-41 token interface compliance ─────────────────────────────────────

    /// The token registered via initialize must be a valid SEP-41 contract
    /// (register_stellar_asset_contract_v2 produces a compliant token).
    #[test]
    fn test_token_is_sep41_compliant() {
        let env = Env::default();
        env.mock_all_auths();
        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        // register_stellar_asset_contract_v2 produces a SEP-41 compliant token
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admins = Vec::from_array(&env, [admin]);
        // initialize succeeds only if token passes SEP-41 validation
        client.initialize(&deployer, &admins, &1u32, &token);
        // If we reach here, the token was accepted as SEP-41 compliant
    }

    /// Passing a non-token address as the token to initialize must be rejected.
    #[test]
    #[should_panic]
    fn test_non_sep41_token_rejected_on_initialize() {
        let env = Env::default();
        env.mock_all_auths();
        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        // A plain generated address is not a SEP-41 token contract
        let fake_token = Address::generate(&env);
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let admins = Vec::from_array(&env, [admin]);
        client.initialize(&deployer, &admins, &1u32, &fake_token);
    }

    // ── contracttype derive (structural) ─────────────────────────────────────

    /// Verify ContractError is Copy + Clone + Eq (required by #[contracterror]).
    #[test]
    fn test_contract_error_is_copy_clone_eq() {
        let e = ContractError::InsufficientFunds;
        let e2 = e; // Copy
        let e3 = e2.clone(); // Clone
        assert_eq!(e, e3); // Eq
    }
}
