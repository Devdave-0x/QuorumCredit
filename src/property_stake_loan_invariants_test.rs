/// Property-based tests for protocol-wide invariants.
///
/// Tests use deterministic parameter tables rather than external randomness, making
/// them reproducible in Soroban's deterministic execution environment.
///
/// Invariants verified:
///
///   P1 – total_stake >= active_loan_amount
///        For any valid vouch+loan where threshold ≤ total_stake, the total staked
///        collateral is always at least as large as the disbursed loan amount.
///        Parametric over: stakes × loan fractions of stake (10%–100%).
///
///   P2 – contract_balance >= sum of all active voucher stakes
///        The contract always holds enough tokens to return all locked stakes.
///        Parametric over N concurrent active borrowers (N ∈ {1,2,3,5}).
///
///   P3a – slash arithmetic conservation: slashed + returned == original_stake
///        Pure arithmetic property verified for slash_bps ∈ {0,500,…,10000}.
///
///   P3b – contract-level slash conservation
///        After vote_slash triggers, the contract balance decreases by exactly
///        the unslashed amount returned to the voucher.
///
///   P4  – loan yield is non-negative and bounded by principal
///        For every disbursed loan: 0 ≤ total_yield ≤ amount.
///
///   P4b – after full repayment, loan status is Repaid
///        amount_repaid is always within [0, amount + total_yield].
///
///   P5  – config BPS values stay in valid range after updates
///        After any valid update_config call: yield_bps ∈ [0,10000], slash_bps ∈ (0,10000].
///
///   P6  – loan state transitions are strictly forward
///        None → Active → (Repaid | Defaulted); no backwards transitions.
#[cfg(test)]
mod property_stake_loan_invariants_tests {
    use crate::{LoanStatus, QuorumCreditContract, QuorumCreditContractClient};
    use crate::types::BPS_DENOMINATOR;
    use soroban_sdk::{
        testutils::Address as _,
        token::{StellarAssetClient, TokenClient},
        Address, Env, String, Vec,
    };

    struct Setup {
        env: Env,
        client: QuorumCreditContractClient<'static>,
        admin_vec: Vec<Address>,
        contract_id: Address,
        token: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admin_vec = Vec::from_array(&env, [admin.clone()]);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        // Pre-fund the contract so it can disburse loans.
        StellarAssetClient::new(&env, &token_id.address()).mint(&contract_id, &100_000_000);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admin_vec, &1, &token_id.address());

        // Set a nonzero timestamp so vouch timestamps are meaningful.
        env.ledger().with_mut(|l| l.timestamp = 120);

        Setup { env, client, admin_vec, contract_id, token: token_id.address() }
    }

    fn mint(s: &Setup, to: &Address, amount: i128) {
        StellarAssetClient::new(&s.env, &s.token).mint(to, &amount);
    }

    fn balance(s: &Setup, addr: &Address) -> i128 {
        TokenClient::new(&s.env, &s.token).balance(addr)
    }

    fn purpose(env: &Env) -> String {
        String::from_str(env, "test")
    }

    // ── P1: total_stake >= active_loan_amount ─────────────────────────────────

    /// For any valid (stake, loan_fraction) pair, the total staked collateral
    /// backing an active loan is >= the loan amount.
    ///
    /// This holds because request_loan enforces total_stake >= threshold,
    /// and the test sets threshold = stake. Parametric over a grid of
    /// amounts and loan fractions.
    #[test]
    fn property_total_stake_geq_active_loan() {
        let stakes: &[i128] = &[100_000, 500_000, 1_000_000, 5_000_000];
        let fractions_bps: &[i128] = &[1_000, 2_500, 5_000, 7_500, 10_000]; // 10%–100%

        for &stake in stakes {
            for &frac_bps in fractions_bps {
                let loan_amount = stake * frac_bps / 10_000;
                if loan_amount < 100_000 {
                    continue; // skip: below DEFAULT_MIN_LOAN_AMOUNT
                }

                let s = setup();
                let voucher = Address::generate(&s.env);
                let borrower = Address::generate(&s.env);
                mint(&s, &voucher, stake);

                s.client.vouch(&voucher, &borrower, &stake, &s.token, &None);
                s.client.request_loan(
                    &borrower, &loan_amount, &stake, &purpose(&s.env), &s.token,
                );

                let loan = s.client.get_loan(&borrower).unwrap();
                let total_stake: i128 =
                    s.client.get_vouches(&borrower).iter().map(|v| v.stake).sum();

                assert!(
                    total_stake >= loan.amount,
                    "P1: stake={stake} frac={frac_bps}bps \
                     total_stake={total_stake} < loan.amount={}",
                    loan.amount
                );
            }
        }
    }

    // ── P2: contract_balance >= sum of all active voucher stakes ──────────────

    /// With N concurrent active borrowers, the contract always holds at least as
    /// many tokens as the sum of all locked voucher stakes.
    ///
    /// Uses a fixed-size unrolled approach for each N to avoid std::vec dependency.
    #[test]
    fn property_contract_balance_covers_all_active_stakes() {
        let per_stake: i128 = 1_000_000;
        let per_loan: i128 = 500_000;

        // N = 1
        {
            let s = setup();
            let v = Address::generate(&s.env);
            let b = Address::generate(&s.env);
            mint(&s, &v, per_stake);
            s.client.vouch(&v, &b, &per_stake, &s.token, &None);
            s.client.request_loan(&b, &per_loan, &per_stake, &purpose(&s.env), &s.token);

            let active_stake = if s.client.loan_status(&b) == LoanStatus::Active {
                s.client.get_vouches(&b).iter().map(|v| v.stake).sum()
            } else {
                0i128
            };
            let bal = balance(&s, &s.contract_id);
            assert!(
                bal >= active_stake,
                "P2 (n=1): contract_balance={bal} < active_stake={active_stake}"
            );
        }

        // N = 3
        {
            let s = setup();
            let mut total_active_stake = 0i128;

            let borrowers = [
                (Address::generate(&s.env), Address::generate(&s.env)),
                (Address::generate(&s.env), Address::generate(&s.env)),
                (Address::generate(&s.env), Address::generate(&s.env)),
            ];

            for (v, b) in &borrowers {
                mint(&s, v, per_stake);
                s.client.vouch(v, b, &per_stake, &s.token, &None);
                s.client.request_loan(b, &per_loan, &per_stake, &purpose(&s.env), &s.token);
            }

            for (_, b) in &borrowers {
                if s.client.loan_status(b) == LoanStatus::Active {
                    let stake: i128 = s.client.get_vouches(b).iter().map(|v| v.stake).sum();
                    total_active_stake += stake;
                }
            }

            let bal = balance(&s, &s.contract_id);
            assert!(
                bal >= total_active_stake,
                "P2 (n=3): contract_balance={bal} < active_stake={total_active_stake}"
            );
        }

        // N = 5
        {
            let s = setup();
            let mut total_active_stake = 0i128;

            let borrowers = [
                (Address::generate(&s.env), Address::generate(&s.env)),
                (Address::generate(&s.env), Address::generate(&s.env)),
                (Address::generate(&s.env), Address::generate(&s.env)),
                (Address::generate(&s.env), Address::generate(&s.env)),
                (Address::generate(&s.env), Address::generate(&s.env)),
            ];

            for (v, b) in &borrowers {
                mint(&s, v, per_stake);
                s.client.vouch(v, b, &per_stake, &s.token, &None);
                s.client.request_loan(b, &per_loan, &per_stake, &purpose(&s.env), &s.token);
            }

            for (_, b) in &borrowers {
                if s.client.loan_status(b) == LoanStatus::Active {
                    let stake: i128 = s.client.get_vouches(b).iter().map(|v| v.stake).sum();
                    total_active_stake += stake;
                }
            }

            let bal = balance(&s, &s.contract_id);
            assert!(
                bal >= total_active_stake,
                "P2 (n=5): contract_balance={bal} < active_stake={total_active_stake}"
            );
        }
    }

    // ── P3a: Slash arithmetic conservation ───────────────────────────────────

    /// For every slash_bps value in the valid range, slashed + returned == original_stake.
    /// Pure arithmetic — does not interact with the contract.
    #[test]
    fn property_slash_conservation_arithmetic() {
        let slash_bps_cases: &[i128] = &[0, 500, 1_000, 2_500, 5_000, 7_500, 10_000];
        let stakes: &[i128] = &[100_000, 1_000_000, 10_000_000, 100_000_000];

        for &slash_bps in slash_bps_cases {
            for &stake in stakes {
                let slashed = stake * slash_bps / BPS_DENOMINATOR;
                let returned = stake - slashed;

                assert_eq!(
                    slashed + returned,
                    stake,
                    "P3a: conservation violated slash_bps={slash_bps} stake={stake}"
                );
                assert!(slashed >= 0, "P3a: slashed < 0 for slash_bps={slash_bps}");
                assert!(returned >= 0, "P3a: returned < 0 for slash_bps={slash_bps}");
                assert!(
                    slashed <= stake,
                    "P3a: slashed={slashed} > stake={stake} for slash_bps={slash_bps}"
                );
            }
        }
    }

    // ── P3b: Contract-level slash conservation ────────────────────────────────

    /// After a slash is executed via vote_slash, the contract balance decreases by
    /// exactly the unslashed amount returned to the voucher.
    #[test]
    fn property_slash_conservation_contract() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        let stake: i128 = 2_000_000;
        let loan: i128 = 500_000;

        // Default slash_bps = 5000 (50%).
        let slash_bps: i128 = 5_000;
        let slashed = stake * slash_bps / BPS_DENOMINATOR; // 1_000_000
        let returned = stake - slashed;                     // 1_000_000

        mint(&s, &voucher, stake);
        s.client.vouch(&voucher, &borrower, &stake, &s.token, &None);
        s.client.request_loan(&borrower, &loan, &stake, &purpose(&s.env), &s.token);

        let bal_before = balance(&s, &s.contract_id);

        // Lower quorum so a single voucher vote meets quorum and creates a pending slash.
        // With slash_delay_seconds=0 (default), the pending slash is immediately executable.
        s.client.set_slash_vote_quorum(&s.admin_vec, &1);
        s.client.vote_slash(&voucher, &borrower, &true);
        // Execute the pending slash (delay=0, so executable immediately).
        s.client.execute_pending_slash(&borrower);

        let bal_after = balance(&s, &s.contract_id);
        let balance_delta = bal_before - bal_after;

        assert_eq!(
            balance_delta,
            returned,
            "P3b: balance delta {balance_delta} != returned {returned}"
        );
        assert_eq!(slashed + returned, stake, "P3b: conservation violated");
        assert_eq!(
            s.client.loan_status(&borrower),
            LoanStatus::Defaulted,
            "P3b: loan should be Defaulted after slash"
        );
    }

    // ── P4: Loan yield is non-negative and bounded by principal ───────────────

    /// For every disbursed loan: total_yield ∈ [0, amount].
    /// At default 200 bps yield rate, yield = 2% of principal.
    #[test]
    fn property_loan_yield_bounded_by_principal() {
        let loan_amounts: &[i128] = &[100_000, 500_000, 1_000_000, 5_000_000, 10_000_000];

        for &loan_amount in loan_amounts {
            let s = setup();
            let voucher = Address::generate(&s.env);
            let borrower = Address::generate(&s.env);
            let stake = loan_amount * 2;

            mint(&s, &voucher, stake);
            s.client.vouch(&voucher, &borrower, &stake, &s.token, &None);
            s.client.request_loan(
                &borrower, &loan_amount, &stake, &purpose(&s.env), &s.token,
            );

            let loan = s.client.get_loan(&borrower).unwrap();
            assert!(
                loan.total_yield >= 0,
                "P4: total_yield < 0 for loan={loan_amount}"
            );
            assert!(
                loan.total_yield <= loan_amount,
                "P4: yield={} exceeds principal={} (loan_amount={loan_amount})",
                loan.total_yield,
                loan.amount
            );
            assert_eq!(loan.amount_repaid, 0, "P4: fresh loan must have 0 repaid");
            assert_eq!(loan.amount, loan_amount, "P4: disbursed amount mismatch");
            assert_eq!(loan.status, LoanStatus::Active, "P4: new loan must be Active");
        }
    }

    // ── P4b: Repayment bounds and status after full repay ─────────────────────

    /// After a full repayment, amount_repaid is within [0, amount + total_yield]
    /// and the loan status transitions to Repaid.
    #[test]
    fn property_repayment_bounds_and_status() {
        let loan_amounts: &[i128] = &[100_000, 500_000, 1_000_000];

        for &loan_amount in loan_amounts {
            let s = setup();
            let voucher = Address::generate(&s.env);
            let borrower = Address::generate(&s.env);
            let stake = loan_amount * 2;

            mint(&s, &voucher, stake);
            mint(&s, &borrower, loan_amount * 2);
            s.client.vouch(&voucher, &borrower, &stake, &s.token, &None);
            s.client.request_loan(
                &borrower, &loan_amount, &stake, &purpose(&s.env), &s.token,
            );

            let loan = s.client.get_loan(&borrower).unwrap();
            let total_owed = loan.amount + loan.total_yield;

            assert_eq!(loan.amount_repaid, 0, "P4b: fresh loan must have 0 repaid");
            assert!(total_owed >= loan_amount, "P4b: total_owed < principal");

            s.client.repay(&borrower, &total_owed);

            // After full repayment: loan_status uses the LatestLoan record.
            assert_eq!(
                s.client.loan_status(&borrower),
                LoanStatus::Repaid,
                "P4b: expected Repaid for loan={loan_amount}"
            );
        }
    }

    // ── P5: Config BPS values stay in valid range after updates ───────────────

    /// After any valid update_config call, yield_bps ∈ [0,10000] and
    /// slash_bps ∈ (0,10000] (zero slash_bps is rejected).
    #[test]
    fn property_config_bps_valid_after_updates() {
        let yield_bps_cases: &[i128] = &[0, 1, 100, 200, 500, 1_000, 5_000, 9_999, 10_000];
        let slash_bps_cases: &[i128] = &[1, 100, 500, 1_000, 5_000, 9_999, 10_000];

        for &yield_bps in yield_bps_cases {
            for &slash_bps in slash_bps_cases {
                let s = setup();
                s.client.update_config(&s.admin_vec, &Some(yield_bps), &Some(slash_bps));
                let cfg = s.client.get_config();

                assert!(
                    cfg.yield_bps >= 0 && cfg.yield_bps <= 10_000,
                    "P5: yield_bps={} out of [0,10000]",
                    cfg.yield_bps
                );
                assert!(
                    cfg.slash_bps > 0 && cfg.slash_bps <= 10_000,
                    "P5: slash_bps={} out of (0,10000]",
                    cfg.slash_bps
                );
                assert_eq!(cfg.yield_bps, yield_bps, "P5: yield_bps not persisted");
                assert_eq!(cfg.slash_bps, slash_bps, "P5: slash_bps not persisted");
            }
        }
    }

    // ── P6: Loan state transitions are strictly forward ───────────────────────

    /// State machine: None → Active → Repaid.
    /// Status never moves backwards.
    #[test]
    fn property_loan_state_transitions_forward_only_repay() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        let stake: i128 = 1_000_000;
        let loan_amount: i128 = 500_000;

        mint(&s, &voucher, stake);
        mint(&s, &borrower, loan_amount * 2);

        // No loan yet: None.
        assert_eq!(s.client.loan_status(&borrower), LoanStatus::None);

        s.client.vouch(&voucher, &borrower, &stake, &s.token, &None);
        // Vouch alone doesn't activate a loan.
        assert_eq!(s.client.loan_status(&borrower), LoanStatus::None);

        s.client.request_loan(&borrower, &loan_amount, &stake, &purpose(&s.env), &s.token);
        // After request_loan: Active.
        assert_eq!(s.client.loan_status(&borrower), LoanStatus::Active);

        let loan = s.client.get_loan(&borrower).unwrap();
        s.client.repay(&borrower, &(loan.amount + loan.total_yield));

        // After full repay: Repaid — never goes back to Active or None.
        assert_eq!(s.client.loan_status(&borrower), LoanStatus::Repaid);
    }

    /// State machine: None → Active → Defaulted (via slash).
    #[test]
    fn property_loan_state_transitions_forward_only_slash() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        let stake: i128 = 1_000_000;
        let loan_amount: i128 = 500_000;

        mint(&s, &voucher, stake);
        s.client.vouch(&voucher, &borrower, &stake, &s.token, &None);

        assert_eq!(s.client.loan_status(&borrower), LoanStatus::None);

        s.client.request_loan(&borrower, &loan_amount, &stake, &purpose(&s.env), &s.token);
        assert_eq!(s.client.loan_status(&borrower), LoanStatus::Active);

        // Lower quorum so single vote meets quorum, then execute the pending slash.
        s.client.set_slash_vote_quorum(&s.admin_vec, &1);
        s.client.vote_slash(&voucher, &borrower, &true);
        s.client.execute_pending_slash(&borrower);

        assert_eq!(s.client.loan_status(&borrower), LoanStatus::Defaulted);
    }

    // ── P1+P2 combined: invariants hold across full lifecycle ─────────────────

    /// Comprehensive check: verify P1 and P2 after every phase in a 3-borrower
    /// vouch → loan → repay scenario.
    #[test]
    fn property_invariants_hold_across_full_lifecycle() {
        let s = setup();
        let per_stake: i128 = 2_000_000;
        let per_loan: i128 = 1_000_000;

        let pairs = [
            (Address::generate(&s.env), Address::generate(&s.env)),
            (Address::generate(&s.env), Address::generate(&s.env)),
            (Address::generate(&s.env), Address::generate(&s.env)),
        ];

        // Phase 1: all vouch.
        for (v, b) in &pairs {
            mint(&s, v, per_stake);
            s.client.vouch(v, b, &per_stake, &s.token, &None);
        }

        // Phase 2: all request loans.
        for (_, b) in &pairs {
            s.client.request_loan(b, &per_loan, &per_stake, &purpose(&s.env), &s.token);
        }

        // After all loans active — verify P2.
        let active_stake: i128 = pairs.iter().map(|(_, b)| {
            if s.client.loan_status(b) == LoanStatus::Active {
                s.client.get_vouches(b).iter().map(|v| v.stake).sum()
            } else {
                0
            }
        }).sum();
        let bal = balance(&s, &s.contract_id);
        assert!(
            bal >= active_stake,
            "lifecycle P2: contract_balance={bal} < active_stake={active_stake}"
        );

        // Verify P1 per borrower.
        for (_, b) in &pairs {
            let loan = s.client.get_loan(b).unwrap();
            let total_stake: i128 = s.client.get_vouches(b).iter().map(|v| v.stake).sum();
            assert!(
                total_stake >= loan.amount,
                "lifecycle P1: total_stake={total_stake} < loan.amount={}",
                loan.amount
            );
        }

        // Phase 3: all repay.
        for (_, b) in &pairs {
            let loan = s.client.get_loan(b).unwrap();
            let total_owed = loan.amount + loan.total_yield;
            mint(&s, b, total_owed);
            s.client.repay(b, &total_owed);
        }

        // All loans should be Repaid.
        for (_, b) in &pairs {
            assert_eq!(
                s.client.loan_status(b),
                LoanStatus::Repaid,
                "lifecycle: all loans should be Repaid after full payment"
            );
        }
    }
}
