/// Documentation tests verifying the README code examples work correctly.
///
/// These tests mirror the conversion helpers and constants documented in the
/// README "Stroop Unit Convention" section.
#[cfg(test)]
mod doc_tests {
    use crate::types::{
        BPS_DENOMINATOR, DEFAULT_MIN_LOAN_AMOUNT, DEFAULT_MIN_YIELD_STAKE, DEFAULT_YIELD_BPS,
    };

    // Conversion helpers from README (Rust section)
    fn xlm_to_stroops(xlm: f64) -> i128 {
        (xlm * 10_000_000.0) as i128
    }

    fn stroops_to_xlm(stroops: i128) -> f64 {
        stroops as f64 / 10_000_000.0
    }

    #[test]
    fn test_xlm_to_stroops() {
        assert_eq!(xlm_to_stroops(1.0), 10_000_000);
        assert_eq!(xlm_to_stroops(0.01), 100_000);
        assert_eq!(xlm_to_stroops(0.0), 0);
    }

    #[test]
    fn test_stroops_to_xlm() {
        assert_eq!(stroops_to_xlm(10_000_000), 1.0);
        assert_eq!(stroops_to_xlm(100_000), 0.01);
        assert_eq!(stroops_to_xlm(0), 0.0);
    }

    #[test]
    fn test_round_trip_conversion() {
        let xlm = 5.0_f64;
        assert_eq!(stroops_to_xlm(xlm_to_stroops(xlm)), xlm);
    }

    #[test]
    fn test_constants() {
        assert_eq!(BPS_DENOMINATOR, 10_000);
        assert_eq!(DEFAULT_YIELD_BPS, 200); // 2%
        assert_eq!(DEFAULT_MIN_LOAN_AMOUNT, 100_000); // 0.01 XLM
        assert_eq!(DEFAULT_MIN_YIELD_STAKE, 50);
    }

    #[test]
    fn test_yield_calculation() {
        // 2% yield on 1 XLM stake = 200_000 stroops
        let stake = xlm_to_stroops(1.0);
        let yield_amount = stake * DEFAULT_YIELD_BPS / BPS_DENOMINATOR;
        assert_eq!(yield_amount, 200_000);
    }

    #[test]
    fn test_min_yield_stake_boundary() {
        // Stake below DEFAULT_MIN_YIELD_STAKE truncates to zero yield at 2%
        let below = DEFAULT_MIN_YIELD_STAKE - 1; // 49 stroops
        let yield_below = below * DEFAULT_YIELD_BPS / BPS_DENOMINATOR;
        assert_eq!(yield_below, 0);

        // At exactly DEFAULT_MIN_YIELD_STAKE, yield is non-zero
        let at_min = DEFAULT_MIN_YIELD_STAKE; // 50 stroops
        let yield_at_min = at_min * DEFAULT_YIELD_BPS / BPS_DENOMINATOR;
        assert_eq!(yield_at_min, 1);
    }
}
