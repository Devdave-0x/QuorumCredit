# Contract Upgrade Guide

This guide documents the procedures and safety checks for upgrading the QuorumCredit contract on Stellar mainnet.

## Overview

Contract upgrades allow patching vulnerabilities and adding features without redeploying. The upgrade process requires:
1. Admin multisig approval (≥ `admin_threshold` signatures)
2. Pre-upgrade safety checks
3. Pause/unpause cycle for data consistency
4. Post-upgrade verification

## Pre-Upgrade Checklist

### Code Review & Testing

- [ ] All changes reviewed by ≥2 team members
- [ ] Unit tests passing: `cargo test`
- [ ] Integration tests passing: `cargo test --test '*'`
- [ ] Code coverage ≥ 80%
- [ ] No clippy warnings: `cargo clippy`
- [ ] Security audit completed (if applicable)

### Testnet Validation

- [ ] Deployed to testnet
- [ ] Upgrade tested on testnet
- [ ] Monitored for 24+ hours
- [ ] No errors or anomalies observed
- [ ] Rollback tested and verified

### Mainnet Preparation

- [ ] Backup current contract state
- [ ] Document current configuration
- [ ] Notify users of maintenance window
- [ ] Prepare rollback plan
- [ ] Ensure admin keys are accessible

## Upgrade Procedure

### Step 1: Build New WASM

```bash
cd QuorumCredit
cargo build --target wasm32-unknown-unknown --release

# Verify build
ls -lh target/wasm32-unknown-unknown/release/quorum_credit.wasm

# Calculate WASM hash for verification
sha256sum target/wasm32-unknown-unknown/release/quorum_credit.wasm
```

### Step 2: Pause Contract

Pause the contract to prevent state mutations during upgrade:

```bash
# Admin 1 initiates pause
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn pause \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]'

# Verify paused
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet | grep -i paused
```

### Step 3: Install New WASM

Upload the new WASM code to the network:

```bash
NEW_WASM_HASH=$(stellar contract install \
  --wasm target/wasm32-unknown-unknown/release/quorum_credit.wasm \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY)

echo "New WASM hash: $NEW_WASM_HASH"

# Verify hash matches local build
echo "Expected: $(sha256sum target/wasm32-unknown-unknown/release/quorum_credit.wasm | cut -d' ' -f1)"
```

### Step 4: Execute Upgrade

Upgrade the contract to use the new WASM:

```bash
# Admin 1 initiates upgrade
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn upgrade \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]' \
  --new_wasm_hash $NEW_WASM_HASH

# Wait for confirmation (typically 5-10 seconds)
sleep 10

# Verify upgrade succeeded
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet
```

### Step 5: Unpause Contract

Resume normal operations:

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn unpause \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]'

# Verify unpaused
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet | grep -i paused
```

## Safety Checks

### Pre-Upgrade Validation

Before pausing, verify contract health:

```bash
# Check active loans
ACTIVE_LOANS=$(stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet | grep -oP 'active_loans": \K\d+')

echo "Active loans: $ACTIVE_LOANS"

# Check yield reserve
YIELD_RESERVE=$(stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_fee_treasury \
  --network mainnet)

echo "Yield reserve: $YIELD_RESERVE stroops"

# Check contract balance
CONTRACT_BALANCE=$(stellar account info $CONTRACT_ID --network mainnet | grep -oP 'Balance: \K[\d.]+')

echo "Contract balance: $CONTRACT_BALANCE XLM"
```

### Post-Upgrade Validation

After upgrade, verify contract integrity:

```bash
# Verify config unchanged
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet

# Verify admins unchanged
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admins \
  --network mainnet

# Test basic operations
# 1. Query a loan
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_loan \
  --network mainnet \
  -- \
  --borrower $TEST_BORROWER

# 2. Query vouches
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_vouches \
  --network mainnet \
  -- \
  --borrower $TEST_BORROWER

# 3. Check eligibility
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn is_eligible \
  --network mainnet \
  -- \
  --borrower $TEST_BORROWER \
  --threshold 1000000000 \
  --token_addr $TOKEN_CONTRACT
```

### Monitoring During Upgrade

Monitor metrics during the upgrade window:

```bash
# Watch error rate
watch -n 5 'curl -s http://prometheus:9090/api/v1/query?query=rate\(qc_contract_errors_total\[5m\]\) | jq'

# Watch transaction latency
watch -n 5 'curl -s http://prometheus:9090/api/v1/query?query=histogram_quantile\(0.95,qc_transaction_latency_ms\) | jq'

# Watch active loans
watch -n 5 'curl -s http://prometheus:9090/api/v1/query?query=qc_active_loans | jq'
```

## Rollback Procedure

If upgrade fails or causes issues:

### Immediate Actions

1. **Pause contract** (if not already paused)
   ```bash
   stellar contract invoke \
     --id $CONTRACT_ID \
     --fn pause \
     --network mainnet \
     --source $ADMIN_1_SECRET_KEY \
     -- \
     --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]'
   ```

2. **Notify users** of issue and maintenance window

3. **Assess damage** - check logs and metrics

### Rollback Steps

```bash
# 1. Get previous WASM hash from deployment records
PREVIOUS_WASM_HASH="<hash from deployment records>"

# 2. Upgrade back to previous version
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn upgrade \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]' \
  --new_wasm_hash $PREVIOUS_WASM_HASH

# 3. Verify rollback
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet

# 4. Unpause
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn unpause \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]'

# 5. Verify operations restored
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_loan \
  --network mainnet \
  -- \
  --borrower $TEST_BORROWER
```

## Upgrade Scenarios

### Scenario 1: Security Patch

**Timeline:** Immediate (emergency)

```bash
# 1. Build patched version
cargo build --target wasm32-unknown-unknown --release

# 2. Skip testnet, go directly to mainnet
# 3. Pause contract
# 4. Install and upgrade
# 5. Unpause
# 6. Monitor closely for 24 hours
```

### Scenario 2: Feature Addition

**Timeline:** Planned (1-2 week notice)

```bash
# 1. Announce upgrade window (1 week before)
# 2. Deploy to testnet
# 3. Test for 1 week
# 4. Schedule mainnet upgrade
# 5. Execute upgrade during low-traffic window
# 6. Monitor for 48 hours
```

### Scenario 3: Configuration Change

**Timeline:** Immediate (no code change)

```bash
# No upgrade needed - use set_config instead
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn set_config \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]' \
  --config '{...}'
```

## Upgrade History

Maintain a log of all upgrades:

```markdown
# Upgrade Log

## Upgrade #1 - 2026-05-29
- **Reason:** Security patch - fix reentrancy vulnerability
- **WASM Hash:** abc123...
- **Duration:** 5 minutes
- **Status:** ✅ Success
- **Verified By:** Admin1, Admin2

## Upgrade #2 - 2026-06-15
- **Reason:** Add multi-token support
- **WASM Hash:** def456...
- **Duration:** 10 minutes
- **Status:** ✅ Success
- **Verified By:** Admin1, Admin2, Admin3
```

## Troubleshooting

### Upgrade Fails: "InvalidStateTransition"

**Cause:** Contract not paused before upgrade

**Solution:**
```bash
# Pause contract first
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn pause \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]'

# Retry upgrade
```

### Upgrade Fails: "UnauthorizedCaller"

**Cause:** Insufficient admin signatures

**Solution:**
```bash
# Ensure all required admins sign
# For 2-of-3 multisig, need 2 signatures
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn upgrade \
  --network mainnet \
  --source $ADMIN_1_SECRET_KEY \
  -- \
  --admin_signers '["'$ADMIN_1'","'$ADMIN_2'"]' \
  --new_wasm_hash $NEW_WASM_HASH
```

### Post-Upgrade: Contract Behaves Unexpectedly

**Diagnosis:**
```bash
# Check if contract is paused
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet | grep paused

# Check error logs
tail -f /var/log/quorum-credit/contract.log

# Check metrics
curl http://prometheus:9090/api/v1/query?query=qc_contract_errors_total
```

**Resolution:**
- If paused, unpause
- If errors, check error codes and investigate
- If critical, execute rollback

## Best Practices

1. **Always test on testnet first** - Never upgrade mainnet without testnet validation
2. **Use pause/unpause** - Prevents state inconsistencies during upgrade
3. **Maintain WASM hashes** - Keep records of all deployed WASM hashes for rollback
4. **Monitor closely** - Watch metrics for 24+ hours after upgrade
5. **Document everything** - Log all upgrades with reason, time, and verifier
6. **Have rollback plan** - Always know how to revert to previous version
7. **Communicate with users** - Notify of maintenance windows in advance
8. **Require multisig** - Never allow single-admin upgrades
9. **Verify integrity** - Run post-upgrade validation checks
10. **Keep backups** - Maintain backups of contract state before upgrades
