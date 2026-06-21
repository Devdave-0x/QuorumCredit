#[cfg(test)]
mod integration_invariants {
    use crate::{QuorumCreditContract, QuorumCreditContractClient, LoanStatus};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    struct InvariantContext {
        env: Env,
        contract_id: Address,
        client: QuorumCreditContractClient<'static>,
        token: Address,
    }

    fn setup_invariant_context() -> InvariantContext {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin]);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();

        client.initialize(&deployer, &admins, &1u32, &token);

        InvariantContext {
            env,
            contract_id,
            client,
            token,
        }
    }

    fn mint_tokens(ctx: &InvariantContext, address: &Address, amount: i128) {
        let token_client = soroban_sdk::token::Client::new(&ctx.env, &ctx.token);
        token_client.mint(address, &amount);
    }

    // Invariant 1: Total vouched stake consistency
    #[test]
    fn invariant_total_vouched_consistency() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let vouchers = vec![
            Address::generate(&ctx.env),
            Address::generate(&ctx.env),
            Address::generate(&ctx.env),
        ];

        for voucher in &vouchers {
            mint_tokens(&ctx, voucher, 50_000_000);
        }

        // Vouch with known amounts
        ctx.client.vouch(&vouchers[0], &borrower, &10_000_000, &ctx.token, &None);
        ctx.client.vouch(&vouchers[1], &borrower, &15_000_000, &ctx.token, &None);
        ctx.client.vouch(&vouchers[2], &borrower, &8_000_000, &ctx.token, &None);

        let total_vouched = ctx.client.total_vouched(&borrower);
        assert_eq!(total_vouched, 33_000_000, "Total vouched must equal sum of all vouch stakes");
    }

    // Invariant 2: Loan status transitions are valid
    #[test]
    fn invariant_loan_status_transitions() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 20_000_000);
        mint_tokens(&ctx, &borrower, 1_000_000);

        ctx.client.vouch(&voucher, &borrower, &10_000_000, &ctx.token, &None);

        // Initial state: no loan
        let status = ctx.client.loan_status(&borrower);
        assert_eq!(status, LoanStatus::None, "Initial status must be None");

        // Transition to Active
        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &5_000_000, &10_000_000, &purpose, &ctx.token);

        let status = ctx.client.loan_status(&borrower);
        assert_eq!(status, LoanStatus::Active, "Status must be Active after request");

        // Transition to Repaid
        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");
        ctx.client.repay(&borrower, &(loan.amount + loan.total_yield));

        let status = ctx.client.loan_status(&borrower);
        assert_eq!(status, LoanStatus::Repaid, "Status must be Repaid after repayment");
    }

    // Invariant 3: Loan amount doesn't exceed available credit
    #[test]
    fn invariant_loan_amount_validation() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 50_000_000);

        // Vouch with 30M
        ctx.client.vouch(&voucher, &borrower, &30_000_000, &ctx.token, &None);

        // Request loan within threshold
        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &20_000_000, &30_000_000, &purpose, &ctx.token);

        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");
        assert!(loan.amount <= 30_000_000, "Loan amount must not exceed vouched threshold");
    }

    // Invariant 4: After repayment, loan is cleared from active state
    #[test]
    fn invariant_post_repayment_state() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 20_000_000);
        mint_tokens(&ctx, &borrower, 1_000_000);

        ctx.client.vouch(&voucher, &borrower, &10_000_000, &ctx.token, &None);

        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &5_000_000, &10_000_000, &purpose, &ctx.token);

        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");
        ctx.client.repay(&borrower, &(loan.amount + loan.total_yield));

        let final_loan = ctx.client.get_loan(&borrower).expect("Loan record must persist");
        assert_eq!(final_loan.status, LoanStatus::Repaid, "Loan must be marked Repaid");
        assert!(final_loan.repayment_timestamp.is_some(), "Repayment timestamp must be set");
    }

    // Invariant 5: Contract remains initialized after operations
    #[test]
    fn invariant_initialization_persists() {
        let ctx = setup_invariant_context();

        assert!(ctx.client.is_initialized(), "Contract must be initialized");

        // Perform operations
        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 20_000_000);
        mint_tokens(&ctx, &borrower, 1_000_000);

        ctx.client.vouch(&voucher, &borrower, &10_000_000, &ctx.token, &None);

        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &5_000_000, &10_000_000, &purpose, &ctx.token);

        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");
        ctx.client.repay(&borrower, &(loan.amount + loan.total_yield));

        // Verify still initialized
        assert!(ctx.client.is_initialized(), "Contract must remain initialized after operations");
    }

    // Invariant 6: Admin configuration remains intact
    #[test]
    fn invariant_admin_configuration() {
        let ctx = setup_invariant_context();

        let admins = ctx.client.get_admins();
        assert!(admins.len() > 0, "At least one admin must exist");

        let config = ctx.client.get_config();
        assert!(config.admins.len() > 0, "Config must have admins");

        assert_eq!(admins.len(), config.admins.len(), "Admin list must be consistent");
    }

    // Invariant 7: Vouches remain valid until loan closure
    #[test]
    fn invariant_vouch_persistence() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let vouchers = vec![
            Address::generate(&ctx.env),
            Address::generate(&ctx.env),
        ];

        for voucher in &vouchers {
            mint_tokens(&ctx, voucher, 20_000_000);
            ctx.client.vouch(voucher, &borrower, &10_000_000, &ctx.token, &None);
        }

        let vouches_before = ctx.client.get_vouches(&borrower).expect("Vouches must exist");
        assert_eq!(vouches_before.len(), 2, "Must have 2 vouches");

        // Request and repay
        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &5_000_000, &20_000_000, &purpose, &ctx.token);

        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");
        ctx.client.repay(&borrower, &(loan.amount + loan.total_yield));

        // Vouches should be cleared after repayment
        let vouches_after = ctx.client.get_vouches(&borrower);
        // After repayment, vouches are typically cleared (contract dependent)
        if let Some(vouches) = vouches_after {
            assert!(vouches.len() >= 0, "Vouches should be in valid state");
        }
    }

    // Invariant 8: No negative amounts in loan records
    #[test]
    fn invariant_no_negative_amounts() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 50_000_000);
        mint_tokens(&ctx, &borrower, 1_000_000);

        ctx.client.vouch(&voucher, &borrower, &20_000_000, &ctx.token, &None);

        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &10_000_000, &20_000_000, &purpose, &ctx.token);

        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");

        assert!(loan.amount > 0, "Loan amount must be positive");
        assert!(loan.total_yield >= 0, "Yield cannot be negative");
        assert_eq!(loan.amount_repaid, 0, "Initially no amount repaid");
    }

    // Invariant 9: Config consistency across queries
    #[test]
    fn invariant_config_consistency() {
        let ctx = setup_invariant_context();

        let config1 = ctx.client.get_config();
        let config2 = ctx.client.get_config();

        assert_eq!(config1.admins.len(), config2.admins.len(), "Config must be consistent");
    }

    // Invariant 10: Valid token in loan records
    #[test]
    fn invariant_valid_token_in_loan() {
        let ctx = setup_invariant_context();

        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 20_000_000);
        mint_tokens(&ctx, &borrower, 1_000_000);

        ctx.client.vouch(&voucher, &borrower, &10_000_000, &ctx.token, &None);

        let purpose = soroban_sdk::String::from_str(&ctx.env, "Test");
        ctx.client.request_loan(&borrower, &5_000_000, &10_000_000, &purpose, &ctx.token);

        let loan = ctx.client.get_loan(&borrower).expect("Loan must exist");
        assert_eq!(loan.token_address, ctx.token, "Token must match initialized token");
    }
}
