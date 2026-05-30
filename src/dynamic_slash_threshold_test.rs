#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{calculate_dynamic_slash_threshold, calculate_protocol_health_score};
    use crate::types::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_config(env: &Env, token: Address, dynamic_slash_threshold: bool) -> Config {
        Config {
            admins: soroban_sdk::vec![env, Address::generate(env)],
            admin_threshold: 1,
            token,
            allowed_tokens: soroban_sdk::Vec::new(env),
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
            voting_period_seconds: DEFAULT_VOTING_PERIOD_SECONDS,
            slash_cooldown_seconds: 0,
            emergency_pause_enabled: false,
            dynamic_slash_threshold,
            loan_size_slash_enabled: false,
            loan_size_slash_max_bps: DEFAULT_LOAN_SIZE_SLASH_MAX_BPS,
            confirmation_required: false,
            successor_admin: None,
        }
    }

    #[test]
    fn test_dynamic_slash_threshold_disabled() {
        let env = Env::default();
        env.mock_all_auths();

        let config = make_config(&env, Address::generate(&env), false);
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
        let config = make_config(&env, token_address.clone(), true);
        env.storage().instance().set(&DataKey::Config, &config);

        // Simulate a healthy treasury balance (100 XLM = 1_000_000_000 stroops)
        env.storage()
            .instance()
            .set(&DataKey::SlashTreasury, &1_000_000_000i128);

        let effective_threshold = calculate_dynamic_slash_threshold(&env);

        // Healthy protocol → slash rate should be lower than default (50%)
        // but at or above the minimum (25%)
        assert!(effective_threshold < DEFAULT_SLASH_BPS);
        assert!(effective_threshold >= MIN_DYNAMIC_SLASH_BPS);
    }

    #[test]
    fn test_dynamic_slash_threshold_unhealthy_protocol() {
        let env = Env::default();
        env.mock_all_auths();

        let token_address = Address::generate(&env);
        let config = make_config(&env, token_address.clone(), true);
        env.storage().instance().set(&DataKey::Config, &config);

        // Simulate an unhealthy treasury (0 balance, contract paused)
        env.storage()
            .instance()
            .set(&DataKey::SlashTreasury, &0i128);
        env.storage()
            .instance()
            .set(&DataKey::Paused, &true);

        let effective_threshold = calculate_dynamic_slash_threshold(&env);

        // Unhealthy protocol → slash rate should be higher than default (50%)
        // but at or below the maximum (75%)
        assert!(effective_threshold > DEFAULT_SLASH_BPS);
        assert!(effective_threshold <= MAX_DYNAMIC_SLASH_BPS);
    }

    #[test]
    fn test_protocol_health_score_calculation() {
        let env = Env::default();
        env.mock_all_auths();

        let token_address = Address::generate(&env);
        let config = make_config(&env, token_address.clone(), true);
        env.storage().instance().set(&DataKey::Config, &config);

        // Not paused, excellent treasury balance (100 XLM)
        env.storage()
            .instance()
            .set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .set(&DataKey::SlashTreasury, &1_000_000_000i128);

        let health_score = calculate_protocol_health_score(&env);

        // 3000 (initialized) + 3000 (not paused) + 4000 (excellent balance) = 10000
        assert_eq!(health_score, 10_000);
    }

    #[test]
    fn test_admin_can_toggle_dynamic_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let token_address = Address::generate(&env);

        let mut config = make_config(&env, token_address, false);
        config.admins = soroban_sdk::vec![&env, admin.clone()];
        env.storage().instance().set(&DataKey::Config, &config);

        // Enable dynamic threshold
        crate::admin::set_dynamic_slash_threshold(
            env.clone(),
            soroban_sdk::vec![&env, admin.clone()],
            true,
        );
        let updated: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        assert_eq!(updated.dynamic_slash_threshold, true);

        // Disable it again
        crate::admin::set_dynamic_slash_threshold(
            env.clone(),
            soroban_sdk::vec![&env, admin],
            false,
        );
        let final_config: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        assert_eq!(final_config.dynamic_slash_threshold, false);
    }
}
