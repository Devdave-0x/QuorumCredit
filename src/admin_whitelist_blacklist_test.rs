#[cfg(test)]
mod admin_whitelist_blacklist_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn setup() -> (
        Env,
        QuorumCreditContractClient<'static>,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin1 = Address::generate(&env);
        let admin2 = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let token = env
            .register_stellar_asset_contract_v2(admin1.clone())
            .address();

        let contract_id = env.register(QuorumCreditContract, ());
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &2, &token);

        (env, client, admin1, admin2)
    }

    // ── Issue #688: admin whitelist ───────────────────────────────────────────

    #[test]
    fn test_add_and_remove_admin_whitelist() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let candidate = Address::generate(&env);

        client.add_to_admin_whitelist(&signers, &candidate);
        let cfg = client.get_config();
        assert!(cfg.admin_whitelist.iter().any(|a| a == candidate));

        client.remove_from_admin_whitelist(&signers, &candidate);
        let cfg = client.get_config();
        assert!(!cfg.admin_whitelist.iter().any(|a| a == candidate));
    }

    #[test]
    fn test_whitelist_prevents_non_whitelisted_admin() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let allowed = Address::generate(&env);
        let blocked = Address::generate(&env);

        client.add_to_admin_whitelist(&signers, &allowed);

        let result = client.try_add_admin(&signers, &blocked);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitelisted_admin_can_be_added() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let candidate = Address::generate(&env);

        client.add_to_admin_whitelist(&signers, &candidate);
        client.add_admin(&signers, &candidate);

        let cfg = client.get_config();
        assert!(cfg.admins.iter().any(|a| a == candidate));
    }

    #[test]
    fn test_duplicate_whitelist_entry_rejected() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let candidate = Address::generate(&env);

        client.add_to_admin_whitelist(&signers, &candidate);
        let result = client.try_add_to_admin_whitelist(&signers, &candidate);
        assert!(result.is_err());
    }

    // ── Issue #689: admin blacklist ───────────────────────────────────────────

    #[test]
    fn test_add_and_remove_admin_blacklist() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let bad_actor = Address::generate(&env);

        client.add_to_admin_blacklist(&signers, &bad_actor);
        let cfg = client.get_config();
        assert!(cfg.admin_blacklist.iter().any(|a| a == bad_actor));

        client.remove_from_admin_blacklist(&signers, &bad_actor);
        let cfg = client.get_config();
        assert!(!cfg.admin_blacklist.iter().any(|a| a == bad_actor));
    }

    #[test]
    fn test_blacklisted_address_cannot_be_added_as_admin() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let bad_actor = Address::generate(&env);

        client.add_to_admin_blacklist(&signers, &bad_actor);

        let result = client.try_add_admin(&signers, &bad_actor);
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_blacklist_whitelisted_address() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let candidate = Address::generate(&env);

        client.add_to_admin_whitelist(&signers, &candidate);
        let result = client.try_add_to_admin_blacklist(&signers, &candidate);
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_blacklist_entry_rejected() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let bad_actor = Address::generate(&env);

        client.add_to_admin_blacklist(&signers, &bad_actor);
        let result = client.try_add_to_admin_blacklist(&signers, &bad_actor);
        assert!(result.is_err());
    }

    #[test]
    fn test_removed_from_blacklist_can_be_added_as_admin() {
        let (env, client, admin1, admin2) = setup();
        let signers = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let candidate = Address::generate(&env);

        client.add_to_admin_blacklist(&signers, &candidate);
        client.remove_from_admin_blacklist(&signers, &candidate);
        client.add_admin(&signers, &candidate);

        let cfg = client.get_config();
        assert!(cfg.admins.iter().any(|a| a == candidate));
    }
}
