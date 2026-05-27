# Dynamic Slash Threshold Implementation

## Overview

This implementation adds a dynamic slash threshold feature that adjusts slash penalties based on protocol health. When enabled, the system automatically increases or decreases slash rates to maintain protocol stability.

## Key Features

### 1. **Health-Based Slash Adjustment**
- **Healthy Protocol (≥80% health)**: Lower slash penalty (25-50%)
- **Unhealthy Protocol (<80% health)**: Higher slash penalty (50-75%)
- **Configurable**: Can be enabled/disabled by admins

### 2. **Protocol Health Metrics**
The health score (0-10000 basis points) considers:
- **Initialization Status (30% weight)**: Contract properly initialized
- **Pause State (30% weight)**: Contract not paused
- **Yield Reserve Solvency (40% weight)**: Contract token balance
  - Minimum threshold: 10 XLM (100M stroops)
  - Excellent threshold: 100 XLM (1B stroops)

### 3. **Dynamic Calculation Logic**
```rust
// Health ≥ 80%: Reduce slash penalty
if health_score >= HEALTH_THRESHOLD_BPS {
    // Interpolate between MIN (25%) and static slash_bps (50%)
    slash_bps = cfg.slash_bps - ((cfg.slash_bps - MIN_DYNAMIC_SLASH_BPS) * health_factor / BPS_DENOMINATOR)
} else {
    // Health < 80%: Increase slash penalty  
    // Interpolate between static slash_bps (50%) and MAX (75%)
    slash_bps = cfg.slash_bps + ((MAX_DYNAMIC_SLASH_BPS - cfg.slash_bps) * (BPS_DENOMINATOR - health_factor) / BPS_DENOMINATOR)
}
```

## Implementation Details

### Files Modified

#### 1. **src/types.rs**
- Added `dynamic_slash_threshold: bool` to `Config` struct
- Added constants:
  ```rust
  pub const MIN_DYNAMIC_SLASH_BPS: i128 = 2_500; // 25%
  pub const MAX_DYNAMIC_SLASH_BPS: i128 = 7_500; // 75%  
  pub const HEALTH_THRESHOLD_BPS: i128 = 8_000;  // 80%
  pub const DEFAULT_DYNAMIC_SLASH_THRESHOLD: bool = false;
  ```

#### 2. **src/helpers.rs**
- Added `calculate_dynamic_slash_threshold(env: &Env) -> i128`
- Added `calculate_protocol_health_score(env: &Env) -> i128`

#### 3. **src/governance.rs**
- Modified `execute_slash()` to use dynamic threshold:
  ```rust
  let effective_slash_bps = crate::helpers::calculate_dynamic_slash_threshold(env);
  let slash_amount = loan.amount * voucher_share_bps / BPS_DENOMINATOR * effective_slash_bps / BPS_DENOMINATOR;
  ```
- Enhanced event emission to include both static and dynamic rates for transparency

#### 4. **src/admin.rs**
- Added `set_dynamic_slash_threshold(env, admin_signers, enabled)`
- Added `get_effective_slash_threshold(env) -> i128`

#### 5. **src/contract.rs**
- Added public interface functions:
  - `set_dynamic_slash_threshold()`
  - `get_effective_slash_threshold()`
- Updated `initialize()` to set default `dynamic_slash_threshold: false`

#### 6. **src/dynamic_slash_threshold_test.rs** (New)
- Comprehensive test suite covering:
  - Dynamic threshold disabled/enabled scenarios
  - Healthy vs unhealthy protocol behavior
  - Health score calculation accuracy
  - Admin toggle functionality

## Usage Examples

### Enable Dynamic Slash Threshold
```rust
// Admin enables dynamic threshold
contract.set_dynamic_slash_threshold(admin_signers, true);
```

### Check Current Effective Rate
```rust
// Anyone can check current effective slash rate
let current_rate = contract.get_effective_slash_threshold();
// Returns basis points (e.g., 3750 = 37.5%)
```

### Monitor Protocol Health Impact
```rust
// Before slash execution, the system automatically:
// 1. Calculates protocol health (0-10000 bps)
// 2. Determines effective slash rate based on health
// 3. Applies dynamic rate to slash calculation
// 4. Emits event with both static and dynamic rates
```

## Configuration Parameters

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| `dynamic_slash_threshold` | `false` | `true/false` | Enable/disable dynamic adjustment |
| `MIN_DYNAMIC_SLASH_BPS` | `2500` | N/A | Minimum slash rate (25%) |
| `MAX_DYNAMIC_SLASH_BPS` | `7500` | N/A | Maximum slash rate (75%) |
| `HEALTH_THRESHOLD_BPS` | `8000` | N/A | Health threshold for penalty reduction (80%) |

## Health Score Breakdown

| Component | Weight | Criteria | Score |
|-----------|--------|----------|-------|
| **Initialization** | 30% | Contract initialized | 3000 bps |
| **Pause State** | 30% | Contract not paused | 3000 bps |
| **Solvency** | 40% | Token balance | 0-4000 bps |

### Solvency Scoring
- **0 balance**: 0 bps
- **1-10 XLM**: Linear scale 0-2000 bps
- **10-100 XLM**: Linear scale 2000-4000 bps  
- **100+ XLM**: Full 4000 bps

## Benefits

### 1. **Automatic Risk Management**
- Higher penalties during protocol stress discourage risky behavior
- Lower penalties during healthy periods encourage participation

### 2. **Protocol Stability**
- Responds to liquidity crises by increasing slash deterrent
- Maintains competitive rates during normal operations

### 3. **Transparency**
- Public `get_effective_slash_threshold()` function
- Events include both static and dynamic rates
- Clear health score calculation

### 4. **Administrative Control**
- Can be enabled/disabled by admin consensus
- Preserves existing static slash_bps as fallback
- No breaking changes to existing functionality

## Testing Requirements

Before deployment, run:
```bash
cargo check          # Verify compilation
cargo clippy         # Check code quality  
cargo test           # Run all tests including new dynamic threshold tests
cargo fmt            # Format code
```

## Backward Compatibility

- **Default**: Dynamic threshold disabled (`false`)
- **Existing behavior**: Unchanged when disabled
- **Migration**: No data migration required
- **API**: All existing functions work unchanged

## Security Considerations

1. **Admin Control**: Only admins can toggle dynamic threshold
2. **Bounds Checking**: Slash rates clamped to safe ranges (25-75%)
3. **Health Calculation**: Uses multiple independent metrics
4. **Fallback**: Always falls back to static rate if dynamic disabled
5. **Event Transparency**: All slash events include rate information

## Future Enhancements

1. **Additional Health Metrics**: 
   - Default rate trends
   - Active loan ratios
   - Voucher participation rates

2. **Configurable Parameters**:
   - Admin-adjustable min/max thresholds
   - Custom health weights
   - Time-based smoothing

3. **Advanced Algorithms**:
   - Moving averages for health scores
   - Predictive health modeling
   - Multi-timeframe analysis