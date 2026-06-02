#[cfg(test)]
mod timestamp_validation_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn setup(env: &Env) -> (Address, Vec<Address>, u32, Address) {
        let deployer = Address::generate(env);
        let admin = Address::generate(env);
        let admins = Vec::from_array(env, [admin]);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(env))
            .address();
        (deployer, admins, 1, token)
    }

    #[test]
    fn test_timestamp_tolerance_zero_disables_validation() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let (deployer, admins, threshold, token) = setup(&env);
        client.initialize(&deployer, &admins, &threshold, &token);

        // Get initial config
        let cfg = client.get_config();
        assert_eq!(cfg.timestamp_tolerance_seconds, 300); // Default 5 minutes
    }

    #[test]
    fn test_set_timestamp_tolerance() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let (deployer, admins, threshold, token) = setup(&env);
        client.initialize(&deployer, &admins, &threshold, &token);

        // Set new tolerance
        client.set_timestamp_tolerance(&admins, &600u64);

        let cfg = client.get_config();
        assert_eq!(cfg.timestamp_tolerance_seconds, 600);
    }

    #[test]
    fn test_timestamp_tolerance_within_bounds() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let (deployer, admins, threshold, token) = setup(&env);
        client.initialize(&deployer, &admins, &threshold, &token);

        // Set tolerance to 1 hour
        client.set_timestamp_tolerance(&admins, &3600u64);

        let cfg = client.get_config();
        assert_eq!(cfg.timestamp_tolerance_seconds, 3600);
    }

    #[test]
    fn test_timestamp_tolerance_can_be_disabled() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let (deployer, admins, threshold, token) = setup(&env);
        client.initialize(&deployer, &admins, &threshold, &token);

        // Set tolerance to 0 (disabled)
        client.set_timestamp_tolerance(&admins, &0u64);

        let cfg = client.get_config();
        assert_eq!(cfg.timestamp_tolerance_seconds, 0);
    }

    #[test]
    fn test_only_admin_can_set_timestamp_tolerance() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, QuorumCreditContract);
        let client = QuorumCreditContractClient::new(&env, &contract_id);

        let (deployer, admins, threshold, token) = setup(&env);
        client.initialize(&deployer, &admins, &threshold, &token);

        let non_admin = Address::generate(&env);
        let non_admin_vec = Vec::from_array(&env, [non_admin.clone()]);

        // This should panic because non_admin is not an admin
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.set_timestamp_tolerance(&non_admin_vec, &600u64);
        }));
        assert!(result.is_err(), "non-admin should not be able to set timestamp tolerance");
    }
}
