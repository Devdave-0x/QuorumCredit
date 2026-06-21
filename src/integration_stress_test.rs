#[cfg(test)]
mod integration_stress_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient, LoanStatus};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};
    use std::time::Instant;

    struct StressContext {
        env: Env,
        contract_id: Address,
        client: QuorumCreditContractClient<'static>,
        token: Address,
    }

    fn setup_stress_context() -> StressContext {
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

        StressContext {
            env,
            contract_id,
            client,
            token,
        }
    }

    fn mint_tokens(ctx: &StressContext, address: &Address, amount: i128) {
        let token_client = soroban_sdk::token::Client::new(&ctx.env, &ctx.token);
        token_client.mint(address, &amount);
    }

    // Stress test 1: 100 concurrent borrow operations
    #[test]
    fn stress_100_concurrent_borrows() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let vouchers_count = 20;
        let borrowers_count = 100;

        // Create vouchers
        let mut vouchers = Vec::new();
        for _ in 0..vouchers_count {
            vouchers.push(Address::generate(&ctx.env));
        }

        // Mint large amounts for vouchers
        for voucher in &vouchers {
            mint_tokens(&ctx, voucher, 5_000_000_000); // 500 XLM each
        }

        // Create borrowers and handle loans
        for i in 0..borrowers_count {
            let borrower = Address::generate(&ctx.env);
            let voucher = &vouchers[i % vouchers_count];

            mint_tokens(&ctx, &borrower, 100_000_000); // 10 XLM

            // Vouch
            ctx.client.vouch(voucher, &borrower, &1_000_000, &ctx.token, &None);

            // Request loan
            let purpose = soroban_sdk::String::from_str(&ctx.env, "Stress");
            ctx.client.request_loan(&borrower, &500_000, &1_000_000, &purpose, &ctx.token);

            // Verify loan is active
            assert_eq!(ctx.client.loan_status(&borrower), LoanStatus::Active);

            // Repay to clear the state
            if let Some(loan) = ctx.client.get_loan(&borrower) {
                ctx.client.repay(&borrower, &(loan.amount + loan.total_yield));
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "100 concurrent borrows must complete in under 30 seconds, took {}s",
            elapsed.as_secs()
        );

        println!(
            "✓ Stress Test 1 (100 Concurrent Borrows): {} operations in {}ms",
            borrowers_count,
            elapsed.as_millis()
        );
    }

    // Stress test 2: 50+ vouchers per borrower
    #[test]
    fn stress_50_vouchers_per_borrower() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let borrower = Address::generate(&ctx.env);
        let vouchers_count = 60;

        // Create many vouchers
        let mut vouchers = Vec::new();
        for _ in 0..vouchers_count {
            let voucher = Address::generate(&ctx.env);
            mint_tokens(&ctx, &voucher, 100_000_000); // 10 XLM
            vouchers.push(voucher);
        }

        // Mint for borrower
        mint_tokens(&ctx, &borrower, 1_000_000); // Small amount, not needed for this test

        // Each voucher stakes for the borrower
        let stake_per_voucher = 100_000; // 0.01 XLM per voucher
        for voucher in &vouchers {
            ctx.client.vouch(voucher, &borrower, &stake_per_voucher, &ctx.token, &None);
        }

        // Verify total vouched
        let total_vouched = ctx.client.total_vouched(&borrower);
        assert_eq!(
            total_vouched,
            (vouchers_count as i128) * stake_per_voucher,
            "Total vouched must equal sum of all vouches"
        );

        // Request loan
        let loan_amount = 1_000_000;
        let threshold = 3_000_000;
        let purpose = soroban_sdk::String::from_str(&ctx.env, "50-Voucher");
        ctx.client.request_loan(&borrower, &loan_amount, &threshold, &purpose, &ctx.token);

        assert_eq!(ctx.client.loan_status(&borrower), LoanStatus::Active);

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "50+ vouchers per borrower must complete in under 30 seconds"
        );

        println!(
            "✓ Stress Test 2 (50+ Vouchers): {} vouchers in {}ms",
            vouchers_count,
            elapsed.as_millis()
        );
    }

    // Stress test 3: Sequential loan cycles (rapid repay/borrow)
    #[test]
    fn stress_rapid_loan_cycles() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let borrower = Address::generate(&ctx.env);
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 1_000_000_000); // 100 XLM
        mint_tokens(&ctx, &borrower, 100_000_000); // 10 XLM

        ctx.client.vouch(&voucher, &borrower, &50_000_000, &ctx.token, &None);

        let cycles = 50;
        for _cycle in 0..cycles {
            let purpose = soroban_sdk::String::from_str(&ctx.env, "Cycle");
            ctx.client.request_loan(&borrower, &10_000_000, &50_000_000, &purpose, &ctx.token);

            if let Some(loan) = ctx.client.get_loan(&borrower) {
                ctx.client.repay(&borrower, &(loan.amount + loan.total_yield));
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "50 rapid cycles must complete in under 30 seconds"
        );

        println!(
            "✓ Stress Test 3 (Rapid Cycles): {} cycles in {}ms",
            cycles,
            elapsed.as_millis()
        );
    }

    // Stress test 4: Large volume of queries
    #[test]
    fn stress_high_volume_queries() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let borrowers_count = 100;
        let mut borrowers = Vec::new();
        let voucher = Address::generate(&ctx.env);

        mint_tokens(&ctx, &voucher, 2_000_000_000); // 200 XLM

        // Create borrowers with active loans
        for _ in 0..borrowers_count {
            let borrower = Address::generate(&ctx.env);
            mint_tokens(&ctx, &borrower, 1_000_000);

            ctx.client.vouch(&voucher, &borrower, &10_000_000, &ctx.token, &None);

            let purpose = soroban_sdk::String::from_str(&ctx.env, "Query");
            ctx.client.request_loan(&borrower, &5_000_000, &10_000_000, &purpose, &ctx.token);

            borrowers.push(borrower);
        }

        // Perform many queries
        let query_iterations = 10;
        for _iter in 0..query_iterations {
            for borrower in &borrowers {
                let _status = ctx.client.loan_status(borrower);
                let _loan = ctx.client.get_loan(borrower);
                let _total = ctx.client.total_vouched(borrower);
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "High volume queries must complete in under 30 seconds"
        );

        println!(
            "✓ Stress Test 4 (Query Volume): {} borrowers * {} iterations in {}ms",
            borrowers_count, query_iterations,
            elapsed.as_millis()
        );
    }

    // Stress test 5: State with many active loans
    #[test]
    fn stress_many_active_loans() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let loan_count = 80;
        let mut borrowers = Vec::new();

        // Create many borrowers with active loans
        for i in 0..loan_count {
            let borrower = Address::generate(&ctx.env);
            let voucher = Address::generate(&ctx.env);

            mint_tokens(&ctx, &voucher, 50_000_000); // 5 XLM
            mint_tokens(&ctx, &borrower, 1_000_000);

            ctx.client.vouch(&voucher, &borrower, &10_000_000, &ctx.token, &None);

            let purpose = soroban_sdk::String::from_str(&ctx.env, "Active");
            ctx.client.request_loan(&borrower, &5_000_000, &10_000_000, &purpose, &ctx.token);

            assert_eq!(ctx.client.loan_status(&borrower), LoanStatus::Active);

            borrowers.push(borrower);

            // Every 10 loans, verify we can still query
            if i % 10 == 0 {
                for b in &borrowers {
                    let _status = ctx.client.loan_status(b);
                }
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "Managing {} active loans must complete in under 30 seconds",
            loan_count
        );

        println!(
            "✓ Stress Test 5 (Many Active Loans): {} loans in {}ms",
            loan_count,
            elapsed.as_millis()
        );
    }

    // Stress test 6: Large stake amounts
    #[test]
    fn stress_large_stake_amounts() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let borrower = Address::generate(&ctx.env);
        let vouchers_count = 10;

        let large_amount = 100_000_000_000i128; // 10,000 XLM per voucher

        for _ in 0..vouchers_count {
            let voucher = Address::generate(&ctx.env);
            mint_tokens(&ctx, &voucher, large_amount * 2); // Extra for operations

            ctx.client.vouch(&voucher, &borrower, &large_amount, &ctx.token, &None);
        }

        let total = ctx.client.total_vouched(&borrower);
        assert_eq!(
            total,
            large_amount * vouchers_count as i128,
            "Total vouched should handle large amounts"
        );

        // Request large loan
        let loan_amount = large_amount * 5;
        let threshold = large_amount * vouchers_count as i128;
        let purpose = soroban_sdk::String::from_str(&ctx.env, "Large");
        ctx.client.request_loan(&borrower, &loan_amount, &threshold, &purpose, &ctx.token);

        assert_eq!(ctx.client.loan_status(&borrower), LoanStatus::Active);

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "Large stake amounts must be handled in under 30 seconds"
        );

        println!(
            "✓ Stress Test 6 (Large Amounts): {} stroops in {}ms",
            total,
            elapsed.as_millis()
        );
    }

    // Stress test 7: Mixed operations (vouch, request, repay, query)
    #[test]
    fn stress_mixed_operations() {
        let ctx = setup_stress_context();
        let start = Instant::now();

        let operation_count = 200;
        let mut borrowers = Vec::new();
        let mut vouchers = Vec::new();

        // Setup vouchers
        for _ in 0..20 {
            let voucher = Address::generate(&ctx.env);
            mint_tokens(&ctx, &voucher, 1_000_000_000); // 100 XLM
            vouchers.push(voucher);
        }

        for i in 0..operation_count {
            let op_type = i % 4;

            match op_type {
                0 => {
                    // Vouch operation
                    let borrower = Address::generate(&ctx.env);
                    let voucher = &vouchers[i % vouchers.len()];
                    ctx.client.vouch(voucher, &borrower, &1_000_000, &ctx.token, &None);
                    borrowers.push(borrower);
                }
                1 => {
                    // Request loan
                    if !borrowers.is_empty() {
                        let borrower = &borrowers[i % borrowers.len()];
                        mint_tokens(&ctx, borrower, 100_000);
                        let purpose = soroban_sdk::String::from_str(&ctx.env, "Mixed");
                        let _ = ctx.client.request_loan(borrower, &500_000, &1_000_000, &purpose, &ctx.token);
                    }
                }
                2 => {
                    // Repay
                    if !borrowers.is_empty() {
                        let borrower = &borrowers[i % borrowers.len()];
                        if let Some(loan) = ctx.client.get_loan(borrower) {
                            ctx.client.repay(borrower, &(loan.amount + loan.total_yield));
                        }
                    }
                }
                _ => {
                    // Query
                    if !borrowers.is_empty() {
                        let borrower = &borrowers[i % borrowers.len()];
                        let _status = ctx.client.loan_status(borrower);
                        let _loan = ctx.client.get_loan(borrower);
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "Mixed operations must complete in under 30 seconds"
        );

        println!(
            "✓ Stress Test 7 (Mixed Operations): {} operations in {}ms",
            operation_count,
            elapsed.as_millis()
        );
    }
}
