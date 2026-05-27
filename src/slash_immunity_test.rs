/// Issue #678: Slashing immunity period for new borrowers.
#[cfg(test)]
mod slash_immunity_tests {
    use crate::{
        ContractError, QuorumCreditContract, QuorumCreditContractClient,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env, String, Vec,
    };

    struct Setup {
        env: Env,
        client: QuorumCreditContractClient<'static>,
        admin_vec: Vec<Address>,
        token_id: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);

        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);
        StellarAssetClient::new(&env, &token_id.address()).mint(&contract_id, &50_000_000);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token_id.address());
        env.ledger().with_mut(|l| {
            l.timestamp = crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 5_000
        });

        Setup {
            env,
            client,
            admin_vec: admins,
            token_id: token_id.address(),
        }
    }

    fn purpose(env: &Env) -> String {
        String::from_str(env, "immunity test")
    }

    fn set_immunity(s: &Setup, secs: u64) {
        let mut cfg = s.client.get_config();
        cfg.immunity_period_seconds = secs;
        s.client.set_config(&s.admin_vec, &cfg);
    }

    fn prepare_loan(s: &Setup, borrower: &Address, voucher: &Address) {
        StellarAssetClient::new(&s.env, &s.token_id).mint(voucher, &500_000);
        s.client.vouch(voucher, borrower, &500_000, &s.token_id);
        s.client
            .request_loan(borrower, &100_000, &500_000, &purpose(&s.env), &s.token_id);
        s.client.set_slash_vote_quorum(&s.admin_vec, &1);
    }

    #[test]
    fn test_new_borrower_within_immunity_is_protected() {
        let s = setup();
        set_immunity(&s, 3_600);
        let borrower = Address::generate(&s.env);
        let voucher = Address::generate(&s.env);

        let base = crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 5_000;
        s.env.ledger().with_mut(|l| l.timestamp = base);
        prepare_loan(&s, &borrower, &voucher);

        s.env.ledger().with_mut(|l| l.timestamp = base + 2_000);
        let result = s.client.try_vote_slash(&voucher, &borrower, &true);
        assert_eq!(result, Err(Ok(ContractError::BorrowerImmune)));
    }

    #[test]
    fn test_borrower_past_immunity_can_be_slashed() {
        let s = setup();
        set_immunity(&s, 1_000);
        let borrower = Address::generate(&s.env);
        let voucher = Address::generate(&s.env);

        let base = crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 5_000;
        s.env.ledger().with_mut(|l| l.timestamp = base);
        prepare_loan(&s, &borrower, &voucher);

        s.env.ledger().with_mut(|l| l.timestamp = base + 1_001);
        s.client.vote_slash(&voucher, &borrower, &true);
        assert!(s.client.get_slash_audit(&borrower).is_some());
    }

    #[test]
    fn test_immunity_zero_disables_protection() {
        let s = setup();
        set_immunity(&s, 0);
        let borrower = Address::generate(&s.env);
        let voucher = Address::generate(&s.env);

        let base = crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 5_000;
        s.env.ledger().with_mut(|l| l.timestamp = base);
        prepare_loan(&s, &borrower, &voucher);

        s.client.vote_slash(&voucher, &borrower, &true);
        assert!(s.client.get_slash_audit(&borrower).is_some());
    }

    #[test]
    fn test_boundary_at_end_of_immunity_period() {
        let s = setup();
        set_immunity(&s, 1_000);
        let borrower = Address::generate(&s.env);
        let voucher = Address::generate(&s.env);

        let base = crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 10_000;
        s.env.ledger().with_mut(|l| l.timestamp = base);
        prepare_loan(&s, &borrower, &voucher);

        s.env.ledger().with_mut(|l| l.timestamp = base + 999);
        let blocked = s.client.try_vote_slash(&voucher, &borrower, &true);
        assert_eq!(blocked, Err(Ok(ContractError::BorrowerImmune)));

        s.env.ledger().with_mut(|l| l.timestamp = base + 1_000);
        s.client.vote_slash(&voucher, &borrower, &true);
        assert!(s.client.get_slash_audit(&borrower).is_some());
    }
}
