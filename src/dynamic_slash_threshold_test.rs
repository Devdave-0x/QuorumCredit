#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{calculate_dynamic_slash_threshold, calculate_protocol_health_score};
    use crate::types::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_dynamic_slash_threshold_disabled() {
        let env = Env::default();
        env.mock_all_auths();

        // Initialize with dynamic threshold disabled
        let config = Config {
            admins: vec![&env, Address::generate(&env)],
            admin_threshold: 1,
            token: Address::generate(&env),
            allowed_tokens: Vec::new(&env),
            yield_bps: DEFAULT_YIELD_BPS,
            slash_bps: DEFAULT_SLASH_BPS, // 50%
            max_vouchers: DEFAULT_MAX_VOUCHERS,
            min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
            loan_duration: DEFAULT_LOAN_DURATION,
            max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
            grace_period: 0,
            min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
            prepayment_penalty_bps: 0,
            liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
            dynamic_slash_threshold: false, // Disabled
        };

        env.storage().instance().set(&DataKey::Config, &config);

        // Should return static slash_bps when disabled
        let effective_threshold = calculate_dynamic_slash_threshold(&env);
        assert_eq!(effective_threshold, DEFAULT_SLASH_BPS);
    }

    #[test]
    fn test_dynamic_slash_threshold_healthy_protocol() {
        let env = Env::default();
        env.mock_all_auths();

        let token_address = Address::generate(&env);
        let contract_address = env.current_contract_address();

        // Initialize with dynamic threshold enabled
        let config = Config {
            admins: vec![&env, Address::generate(&env)],
            admin_threshold: 1,
            token: token_address.clone(),
            allowed_tokens: Vec::new(&env),
            yield_bps: DEFAULT_YIELD_BPS,
            slash_bps: DEFAULT_SLASH_BPS, // 50%
            max_vouchers: DEFAULT_MAX_VOUCHERS,
            min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
            loan_duration: DEFAULT_LOAN_DURATION,
            max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
            grace_period: 0,
            min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
            prepayment_penalty_bps: 0,
            liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
            dynamic_slash_threshold: true, // Enabled
        };

        env.storage().instance().set(&DataKey::Config, &config);

        // Mock high contract balance (healthy protocol)
        let token_client = soroban_sdk::token::Client::new(&env, &token_address);
        token_client.mock_all_auths();
        
        // Set contract balance to 100 XLM (1_000_000_000 stroops) - excellent health
        env.as_contract(&contract_address, || {
            env.storage().instance().set(&DataKey::MockBalance, &1_000_000_000i128);
        });

        // Should return lower slash threshold for healthy protocol
        let effective_threshold = calculate_dynamic_slash_threshold(&env);
        
        // Should be lower than default (50%) but higher than minimum (25%)
        assert!(effective_threshold < DEFAULT_SLASH_BPS);
        assert!(effective_threshold >= MIN_DYNAMIC_SLASH_BPS);
    }

    #[test]
    fn test_dynamic_slash_threshold_unhealthy_protocol() {
        let env = Env::default();
        env.mock_all_auths();

        let token_address = Address::generate(&env);

        // Initialize with dynamic threshold enabled
        let config = Config {
            admins: vec![&env, Address::generate(&env)],
            admin_threshold: 1,
            token: token_address.clone(),
            allowed_tokens: Vec::new(&env),
            yield_bps: DEFAULT_YIELD_BPS,
            slash_bps: DEFAULT_SLASH_BPS, // 50%
            max_vouchers: DEFAULT_MAX_VOUCHERS,
            min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
            loan_duration: DEFAULT_LOAN_DURATION,
            max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
            grace_period: 0,
            min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
            prepayment_penalty_bps: 0,
            liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
            dynamic_slash_threshold: true, // Enabled
        };

        env.storage().instance().set(&DataKey::Config, &config);

        // Mock low contract balance (unhealthy protocol)
        // Set contract balance to 1 XLM (10_000_000 stroops) - poor health
        let contract_address = env.current_contract_address();
        env.as_contract(&contract_address, || {
            env.storage().instance().set(&DataKey::MockBalance, &10_000_000i128);
        });

        // Should return higher slash threshold for unhealthy protocol
        let effective_threshold = calculate_dynamic_slash_threshold(&env);
        
        // Should be higher than default (50%) but lower than maximum (75%)
        assert!(effective_threshold > DEFAULT_SLASH_BPS);
        assert!(effective_threshold <= MAX_DYNAMIC_SLASH_BPS);
    }

    #[test]
    fn test_protocol_health_score_calculation() {
        let env = Env::default();
        env.mock_all_auths();

        let token_address = Address::generate(&env);

        // Initialize config
        let config = Config {
            admins: vec![&env, Address::generate(&env)],
            admin_threshold: 1,
            token: token_address.clone(),
            allowed_tokens: Vec::new(&env),
            yield_bps: DEFAULT_YIELD_BPS,
            slash_bps: DEFAULT_SLASH_BPS,
            max_vouchers: DEFAULT_MAX_VOUCHERS,
            min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
            loan_duration: DEFAULT_LOAN_DURATION,
            max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
            grace_period: 0,
            min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
            prepayment_penalty_bps: 0,
            liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
            dynamic_slash_threshold: true,
        };

        env.storage().instance().set(&DataKey::Config, &config);

        // Test with contract not paused and good balance
        env.storage().instance().set(&DataKey::Paused, &false);
        
        // Mock excellent balance (100 XLM)
        let contract_address = env.current_contract_address();
        env.as_contract(&contract_address, || {
            env.storage().instance().set(&DataKey::MockBalance, &1_000_000_000i128);
        });

        let health_score = calculate_protocol_health_score(&env);
        
        // Should be maximum health (10000 basis points = 100%)
        // 3000 (initialized) + 3000 (not paused) + 4000 (excellent balance) = 10000
        assert_eq!(health_score, 10_000);
    }

    #[test]
    fn test_admin_can_toggle_dynamic_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_address = Address::generate(&env);

        // Initialize config with dynamic threshold disabled
        let mut config = Config {
            admins: vec![&env, admin.clone()],
            admin_threshold: 1,
            token: token_address,
            allowed_tokens: Vec::new(&env),
            yield_bps: DEFAULT_YIELD_BPS,
            slash_bps: DEFAULT_SLASH_BPS,
            max_vouchers: DEFAULT_MAX_VOUCHERS,
            min_loan_amount: DEFAULT_MIN_LOAN_AMOUNT,
            loan_duration: DEFAULT_LOAN_DURATION,
            max_loan_to_stake_ratio: DEFAULT_MAX_LOAN_TO_STAKE_RATIO,
            grace_period: 0,
            min_vouch_age_secs: DEFAULT_MIN_VOUCH_AGE_SECS,
            prepayment_penalty_bps: 0,
            liquidity_mining_rate_bps: DEFAULT_LIQUIDITY_MINING_RATE_BPS,
            dynamic_slash_threshold: false,
        };

        env.storage().instance().set(&DataKey::Config, &config);

        // Enable dynamic threshold
        crate::admin::set_dynamic_slash_threshold(env.clone(), vec![&env, admin.clone()], true);

        // Verify it was enabled
        let updated_config: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        assert_eq!(updated_config.dynamic_slash_threshold, true);

        // Disable it again
        crate::admin::set_dynamic_slash_threshold(env.clone(), vec![&env, admin], false);

        // Verify it was disabled
        let final_config: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        assert_eq!(final_config.dynamic_slash_threshold, false);
    }
}