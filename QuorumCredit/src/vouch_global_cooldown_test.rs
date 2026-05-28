#[cfg(test)]
mod tests {
    use crate::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env,
    };

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let token = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let contract_id = env.register(QuorumCreditContract, ());
        QuorumCreditContractClient::new(&env, &contract_id).initialize(
            &deployer,
            &soroban_sdk::vec![&env, admin.clone()],
            &1u32,
            &token,
        );
        (env, contract_id, admin, token, deployer)
    }

    fn fund(env: &Env, token: &Address, admin: &Address, to: &Address, amount: i128) {
        StellarAssetClient::new(env, token).mint(to, &amount);
        // also fund contract for loan disbursement
        StellarAssetClient::new(env, token).mint(admin, &amount);
    }

    #[test]
    fn test_single_vouch_sets_cooldown() {
        let (env, contract_id, admin, token, _) = setup();
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let voucher = Address::generate(&env);
        let borrower = Address::generate(&env);
        fund(&env, &token, &admin, &voucher, 10_000_000_000);

        // First vouch succeeds
        client.vouch(&voucher, &borrower, &1_000_000, &token);

        // Immediate second vouch for a different borrower must fail with cooldown
        let borrower2 = Address::generate(&env);
        let result = client.try_vouch(&voucher, &borrower2, &1_000_000, &token);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_vouch_cooldown_enforced_once() {
        let (env, contract_id, admin, token, _) = setup();
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let voucher = Address::generate(&env);
        fund(&env, &token, &admin, &voucher, 10_000_000_000);

        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);
        let b3 = Address::generate(&env);

        // batch_vouch with 3 borrowers should succeed (cooldown checked once)
        client.batch_vouch(
            &voucher,
            &soroban_sdk::vec![&env, b1.clone(), b2.clone(), b3.clone()],
            &soroban_sdk::vec![&env, 1_000_000i128, 1_000_000i128, 1_000_000i128],
            &token,
        );

        // Immediately after, another batch_vouch must fail — cooldown is active
        let b4 = Address::generate(&env);
        let result = client.try_batch_vouch(
            &voucher,
            &soroban_sdk::vec![&env, b4.clone()],
            &soroban_sdk::vec![&env, 1_000_000i128],
            &token,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_vouch_allowed_after_cooldown_elapses() {
        let (env, contract_id, admin, token, _) = setup();
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let voucher = Address::generate(&env);
        fund(&env, &token, &admin, &voucher, 10_000_000_000);

        let b1 = Address::generate(&env);
        client.batch_vouch(
            &voucher,
            &soroban_sdk::vec![&env, b1.clone()],
            &soroban_sdk::vec![&env, 1_000_000i128],
            &token,
        );

        // Advance time past the cooldown (24 hours + 1 second)
        env.ledger().set(soroban_sdk::testutils::LedgerInfo {
            timestamp: env.ledger().timestamp() + crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 1,
            ..env.ledger().get()
        });

        let b2 = Address::generate(&env);
        // Should succeed now
        client.batch_vouch(
            &voucher,
            &soroban_sdk::vec![&env, b2.clone()],
            &soroban_sdk::vec![&env, 1_000_000i128],
            &token,
        );
    }

    #[test]
    fn test_single_vouch_allowed_after_cooldown_elapses() {
        let (env, contract_id, admin, token, _) = setup();
        let client = QuorumCreditContractClient::new(&env, &contract_id);
        let voucher = Address::generate(&env);
        fund(&env, &token, &admin, &voucher, 10_000_000_000);

        let b1 = Address::generate(&env);
        client.vouch(&voucher, &b1, &1_000_000, &token);

        env.ledger().set(soroban_sdk::testutils::LedgerInfo {
            timestamp: env.ledger().timestamp() + crate::types::DEFAULT_VOUCH_COOLDOWN_SECS + 1,
            ..env.ledger().get()
        });

        let b2 = Address::generate(&env);
        client.vouch(&voucher, &b2, &1_000_000, &token); // must not panic
    }
}
