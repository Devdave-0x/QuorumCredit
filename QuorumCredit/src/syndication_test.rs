/// Tests for #647 Loan Syndication:
/// - create_syndicate returns incrementing IDs
/// - request_loan with syndicate_id associates the loan
/// - get_syndicate_loans returns the correct loan IDs
/// - multiple loans can join the same syndicate
/// - loans without a syndicate_id are not tracked
#[cfg(test)]
mod syndication_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env, String, Vec,
    };

    struct Setup {
        env: Env,
        client: QuorumCreditContractClient<'static>,
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

        Setup { env, client, token: token.address() }
    }

    fn vouch_and_advance(s: &Setup, voucher: &Address, borrower: &Address, stake: i128) {
        StellarAssetClient::new(&s.env, &s.token).mint(voucher, &stake);
        s.client.vouch(voucher, borrower, &stake, &s.token);
        s.env.ledger().with_mut(|l| l.timestamp += 61);
    }

    #[test]
    fn test_create_syndicate_returns_incrementing_ids() {
        let s = setup();
        let id1 = s.client.create_syndicate();
        let id2 = s.client.create_syndicate();
        let id3 = s.client.create_syndicate();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_get_syndicate_loans_empty_for_new_syndicate() {
        let s = setup();
        let sid = s.client.create_syndicate();
        let loans = s.client.get_syndicate_loans(&sid);
        assert_eq!(loans.len(), 0);
    }

    #[test]
    fn test_loan_associated_with_syndicate() {
        let s = setup();
        let sid = s.client.create_syndicate();

        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher, &borrower, 1_000_000);

        s.client.request_loan(
            &borrower,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "syndicated loan"),
            &s.token,
            &Some(sid),
        );

        let loan = s.client.get_loan(&borrower).unwrap();
        assert_eq!(loan.syndicate_id, Some(sid));

        let syndicate_loans = s.client.get_syndicate_loans(&sid);
        assert_eq!(syndicate_loans.len(), 1);
        assert_eq!(syndicate_loans.get(0).unwrap(), loan.id);
    }

    #[test]
    fn test_multiple_loans_in_same_syndicate() {
        let s = setup();
        let sid = s.client.create_syndicate();

        let voucher1 = Address::generate(&s.env);
        let borrower1 = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher1, &borrower1, 1_000_000);
        s.client.request_loan(
            &borrower1,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "loan 1"),
            &s.token,
            &Some(sid),
        );

        let voucher2 = Address::generate(&s.env);
        let borrower2 = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher2, &borrower2, 1_000_000);
        s.client.request_loan(
            &borrower2,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "loan 2"),
            &s.token,
            &Some(sid),
        );

        let syndicate_loans = s.client.get_syndicate_loans(&sid);
        assert_eq!(syndicate_loans.len(), 2);
    }

    #[test]
    fn test_loan_without_syndicate_not_tracked() {
        let s = setup();
        let sid = s.client.create_syndicate();

        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher, &borrower, 1_000_000);

        // Request loan with no syndicate_id
        s.client.request_loan(
            &borrower,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "no syndicate"),
            &s.token,
            &None,
        );

        let loan = s.client.get_loan(&borrower).unwrap();
        assert_eq!(loan.syndicate_id, None);

        // Syndicate should still be empty
        let syndicate_loans = s.client.get_syndicate_loans(&sid);
        assert_eq!(syndicate_loans.len(), 0);
    }

    #[test]
    fn test_different_syndicates_are_independent() {
        let s = setup();
        let sid1 = s.client.create_syndicate();
        let sid2 = s.client.create_syndicate();

        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        vouch_and_advance(&s, &voucher, &borrower, 1_000_000);

        s.client.request_loan(
            &borrower,
            &100_000,
            &500_000,
            &String::from_str(&s.env, "syndicate 1 loan"),
            &s.token,
            &Some(sid1),
        );

        assert_eq!(s.client.get_syndicate_loans(&sid1).len(), 1);
        assert_eq!(s.client.get_syndicate_loans(&sid2).len(), 0);
    }
}
