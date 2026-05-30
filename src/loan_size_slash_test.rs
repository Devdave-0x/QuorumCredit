#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::calculate_loan_size_slash_bps;
    use crate::types::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_config(env: &Env, loan_size_slash_enabled: bool, max_bps: i128) -> Config {
        Config {
            admins: soroban_sdk::vec![env, Address::generate(env)],
            admin_threshold: 1,
            token: Address::generate(env),
            allowed_tokens: soroban_sdk::Vec::new(env),
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
            voting_period_seconds: DEFAULT_VOTING_PERIOD_SECONDS,
            slash_cooldown_seconds: 0,
            emergency_pause_enabled: false,
            dynamic_slash_threshold: false,
            loan_size_slash_enabled,
            loan_size_slash_max_bps: max_bps,
            confirmation_required: false,
            successor_admin: None,
        }
    }

    // ── Unit tests for the pure calculation ──────────────────────────────────

    #[test]
    fn test_small_loan_uses_base_slash() {
        // A tiny loan (1% of total stake) should be very close to the base slash rate
        let base_bps = 5_000i128; // 50%
        let max_bps = 8_000i128;  // 80%
        let total_stake = 1_000_000i128;
        let loan_amount = 10_000i128; // 1% of stake

        let result = calculate_loan_size_slash_bps(loan_amount, total_stake, base_bps, max_bps);

        // 50% + (80%-50%) * 1% = 50% + 0.3% = 50.3% → 5030 bps
        assert_eq!(result, 5_030);
    }

    #[test]
    fn test_medium_loan_interpolates() {
        // A loan equal to 50% of total stake should land at the midpoint
        let base_bps = 5_000i128; // 50%
        let max_bps = 8_000i128;  // 80%
        let total_stake = 1_000_000i128;
        let loan_amount = 500_000i128; // 50% of stake

        let result = calculate_loan_size_slash_bps(loan_amount, total_stake, base_bps, max_bps);

        // 50% + (80%-50%) * 50% = 50% + 15% = 65% → 6500 bps
        assert_eq!(result, 6_500);
    }

    #[test]
    fn test_large_loan_caps_at_max() {
        // A loan equal to or exceeding total stake should hit the maximum
        let base_bps = 5_000i128;
        let max_bps = 8_000i128;
        let total_stake = 1_000_000i128;
        let loan_amount = 1_000_000i128; // 100% of stake

        let result = calculate_loan_size_slash_bps(loan_amount, total_stake, base_bps, max_bps);
        assert_eq!(result, max_bps);
    }

    #[test]
    fn test_oversized_loan_still_caps_at_max() {
        // A loan larger than total stake should still cap at max_bps
        let base_bps = 5_000i128;
        let max_bps = 8_000i128;
        let total_stake = 1_000_000i128;
        let loan_amount = 2_000_000i128; // 200% of stake

        let result = calculate_loan_size_slash_bps(loan_amount, total_stake, base_bps, max_bps);
        assert_eq!(result, max_bps);
    }

    #[test]
    fn test_zero_stake_returns_base() {
        // Edge case: no stake → return base to avoid division by zero
        let base_bps = 5_000i128;
        let max_bps = 8_000i128;

        let result = calculate_loan_size_slash_bps(100_000, 0, base_bps, max_bps);
        assert_eq!(result, base_bps);
    }

    #[test]
    fn test_zero_loan_returns_base() {
        // Edge case: zero loan amount → return base
        let base_bps = 5_000i128;
        let max_bps = 8_000i128;

        let result = calculate_loan_size_slash_bps(0, 1_000_000, base_bps, max_bps);
        assert_eq!(result, base_bps);
    }

    // ── Integration tests via calculate_effective_slash_bps ──────────────────

    #[test]
    fn test_disabled_returns_static_slash_bps() {
        let env = Env::default();
        env.mock_all_auths();

        let config = make_config(&env, false, DEFAULT_LOAN_SIZE_SLASH_MAX_BPS);
        env.storage().instance().set(&DataKey::Config, &config);

        let result = crate::helpers::calculate_effective_slash_bps(
            &env,
            500_000,   // loan amount
            1_000_000, // total stake
        );

        // Feature disabled → should return static slash_bps (50%)
        assert_eq!(result, DEFAULT_SLASH_BPS);
    }

    #[test]
    fn test_enabled_small_loan_near_base() {
        let env = Env::default();
        env.mock_all_auths();

        let config = make_config(&env, true, 8_000);
        env.storage().instance().set(&DataKey::Config, &config);

        let result = crate::helpers::calculate_effective_slash_bps(
            &env,
            10_000,    // tiny loan (1% of stake)
            1_000_000, // total stake
        );

        // Should be close to base (50%) but slightly above
        assert!(result >= DEFAULT_SLASH_BPS);
        assert!(result < 5_100); // well below midpoint
    }

    #[test]
    fn test_enabled_large_loan_near_max() {
        let env = Env::default();
        env.mock_all_auths();

        let config = make_config(&env, true, 8_000);
        env.storage().instance().set(&DataKey::Config, &config);

        let result = crate::helpers::calculate_effective_slash_bps(
            &env,
            950_000,   // 95% of stake
            1_000_000, // total stake
        );

        // Should be close to max (80%)
        assert!(result > 7_500);
        assert!(result <= 8_000);
    }

    #[test]
    fn test_enabled_full_stake_loan_hits_max() {
        let env = Env::default();
        env.mock_all_auths();

        let config = make_config(&env, true, 8_000);
        env.storage().instance().set(&DataKey::Config, &config);

        let result = crate::helpers::calculate_effective_slash_bps(
            &env,
            1_000_000, // loan == total stake
            1_000_000,
        );

        assert_eq!(result, 8_000);
    }

    // ── Admin control tests ───────────────────────────────────────────────────

    #[test]
    fn test_admin_can_enable_loan_size_slash() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let mut config = make_config(&env, false, DEFAULT_LOAN_SIZE_SLASH_MAX_BPS);
        config.admins = soroban_sdk::vec![&env, admin.clone()];
        env.storage().instance().set(&DataKey::Config, &config);

        crate::admin::set_loan_size_slash_enabled(
            env.clone(),
            soroban_sdk::vec![&env, admin.clone()],
            true,
        );

        let updated: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        assert_eq!(updated.loan_size_slash_enabled, true);
    }

    #[test]
    fn test_admin_can_set_max_bps() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let mut config = make_config(&env, true, DEFAULT_LOAN_SIZE_SLASH_MAX_BPS);
        config.admins = soroban_sdk::vec![&env, admin.clone()];
        env.storage().instance().set(&DataKey::Config, &config);

        crate::admin::set_loan_size_slash_max_bps(
            env.clone(),
            soroban_sdk::vec![&env, admin.clone()],
            7_500,
        );

        let updated: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        assert_eq!(updated.loan_size_slash_max_bps, 7_500);
    }

    #[test]
    #[should_panic]
    fn test_max_bps_cannot_be_below_slash_bps() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let mut config = make_config(&env, true, DEFAULT_LOAN_SIZE_SLASH_MAX_BPS);
        config.admins = soroban_sdk::vec![&env, admin.clone()];
        env.storage().instance().set(&DataKey::Config, &config);

        // slash_bps is 5000 (50%), trying to set max to 4000 (40%) should panic
        crate::admin::set_loan_size_slash_max_bps(
            env.clone(),
            soroban_sdk::vec![&env, admin],
            4_000,
        );
    }

    #[test]
    #[should_panic]
    fn test_max_bps_cannot_exceed_100_percent() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let mut config = make_config(&env, true, DEFAULT_LOAN_SIZE_SLASH_MAX_BPS);
        config.admins = soroban_sdk::vec![&env, admin.clone()];
        env.storage().instance().set(&DataKey::Config, &config);

        crate::admin::set_loan_size_slash_max_bps(
            env.clone(),
            soroban_sdk::vec![&env, admin],
            10_001,
        );
    }

    // ── Monotonicity invariant ────────────────────────────────────────────────

    #[test]
    fn test_slash_rate_increases_monotonically_with_loan_size() {
        // Verify that larger loans always produce >= slash rate than smaller loans
        let base_bps = 5_000i128;
        let max_bps = 8_000i128;
        let total_stake = 1_000_000i128;

        let sizes = [0i128, 100_000, 250_000, 500_000, 750_000, 1_000_000, 1_500_000];
        let mut prev = 0i128;
        for &size in &sizes {
            let rate = calculate_loan_size_slash_bps(size, total_stake, base_bps, max_bps);
            assert!(
                rate >= prev,
                "slash rate should be non-decreasing: size={size}, rate={rate}, prev={prev}"
            );
            prev = rate;
        }
    }
}
