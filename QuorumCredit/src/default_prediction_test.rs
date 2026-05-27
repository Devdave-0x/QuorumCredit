/// Tests for #646 Loan Default Prediction:
/// - get_risk_score returns 0 for a borrower with no history
/// - risk score increases after defaults
/// - get_dynamic_yield_bps / get_dynamic_slash_bps reflect risk
/// - yield_bps and slash_bps stored on LoanRecord are risk-adjusted
#[cfg(test)]
mod default_prediction_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env, String, Vec,
    };

    struct Setup {
        env: Env,
        client: QuorumCreditContractClient<'static>,
        admin: Address,
        token: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);
        let token = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        StellarAssetClient::new(&env, &token.address()).mint(&contract_id, &100_000_000);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token.address());

        env.ledger().with_mut(|l| l.timestamp = 120);

        Setup { env, client, admin, token: token.address() }
    }

    fn vouch_and_advance(s: &Setup, voucher: &Address, borrower: &Address, stake: i128) {
        StellarAssetClient::new(&s.env, &s.token).mint(voucher, &stake);
        s.client.vouch(voucher, borrower, &stake, &s.token);
        s.env.ledger().with_mut(|l| l.timestamp += 61);
    }

    #[test]
    fn test_risk_score_zero_for_new_borrower() {
        let s = setup();
        let borrower = Address::generate(&s.env);
        assert_eq!(s.client.get_risk_score(&borrower), 0);
    }

    #[test]
    fn test_dynamic_yield_bps_at_zero_risk_is_below_base() {
        let s = setup();
        let borrower = Address::generate(&s.env);
        // risk_score = 0 → multiplier = 5_000 → yield = base * 5_000 / 10_000 = base / 2
        let base_yield = s.client.get_config().yield_bps; // 200
        let dynamic = s.client.get_dynamic_yield_bps(&borrower);
        // Should be 0.5× base (clamped to at least 1)
        assert!(dynamic <= base_yield, "zero-risk yield should not exceed base");
        assert!(dynamic >= 1, "yield must be at least 1 bps");
    }

    #[test]
    fn test_dynamic_slash_bps_at_zero_risk_equals_base() {
        let s = setup();
        let borrower = Address::generate(&s.env);
        // risk_score = 0 → multiplier = 10_000 → slash = base * 10_000 / 10_000 = base
        let base_slash = s.client.get_config().slash_bps; // 5000
        let dynamic = s.client.get_dynamic_slash_bps(&borrower);
        assert_eq!(dynamic, base_slash);
    }

    #[test]
    fn test_loan_record_stores_risk_adjusted_rates() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher, &borrower, 1_000_000);

        s.client.request_loan(
            &borrower,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "test"),
            &s.token,
            &None,
        );

        let loan = s.client.get_loan(&borrower).unwrap();
        let cfg = s.client.get_config();

        // yield_bps stored on loan should be the dynamic rate, not necessarily the base
        assert!(loan.yield_bps >= 1, "yield_bps must be positive");
        assert!(loan.yield_bps <= cfg.yield_bps * 2, "yield_bps must not exceed 2× base");
        assert!(loan.slash_bps >= cfg.slash_bps, "slash_bps must be at least base for zero-risk");
        assert!(loan.slash_bps <= 10_000, "slash_bps capped at 100%");
    }

    #[test]
    fn test_dynamic_rates_increase_with_risk() {
        let s = setup();
        let borrower = Address::generate(&s.env);

        // Baseline (no history)
        let yield_before = s.client.get_dynamic_yield_bps(&borrower);
        let slash_before = s.client.get_dynamic_slash_bps(&borrower);

        // Simulate a default by incrementing DefaultCount and LoanCount via a
        // full loan-then-slash cycle.
        let voucher = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher, &borrower, 1_000_000);

        s.client.request_loan(
            &borrower,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "will default"),
            &s.token,
            &None,
        );

        // Advance past deadline so slash is valid
        s.env.ledger().with_mut(|l| l.timestamp += 31 * 24 * 60 * 60);

        // Execute slash via governance vote (single voucher = 100% stake)
        s.client.vote_slash(&voucher, &borrower, &true);

        // After slash, risk score should be > 0
        let risk = s.client.get_risk_score(&borrower);
        assert!(risk > 0, "risk score should be positive after a default");

        let yield_after = s.client.get_dynamic_yield_bps(&borrower);
        let slash_after = s.client.get_dynamic_slash_bps(&borrower);

        assert!(yield_after > yield_before, "yield should increase after default");
        assert!(slash_after >= slash_before, "slash should not decrease after default");
    }
}
