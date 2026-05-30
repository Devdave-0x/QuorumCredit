# Production Deployment Guide

This comprehensive guide covers deploying QuorumCredit to Stellar mainnet with security best practices, operational procedures, and post-deployment monitoring.

## Table of Contents

1. [Pre-Deployment Checklist](#pre-deployment-checklist)
2. [Environment Setup](#environment-setup)
3. [Contract Deployment](#contract-deployment)
4. [Post-Deployment Verification](#post-deployment-verification)
5. [Operational Procedures](#operational-procedures)
6. [Monitoring & Alerting](#monitoring--alerting)
7. [Incident Response](#incident-response)
8. [Upgrade Procedures](#upgrade-procedures)

---

## Pre-Deployment Checklist

Before deploying to mainnet, verify all items:

- [ ] All tests passing: `cargo test`
- [ ] Code review completed and approved
- [ ] Security audit completed
- [ ] Testnet deployment verified and tested
- [ ] Admin keys secured (hardware wallet or multisig)
- [ ] Mainnet XLM allocated for deployment fees
- [ ] Monitoring infrastructure ready
- [ ] Incident response plan documented
- [ ] Backup and recovery procedures tested
- [ ] Rate limiting configured
- [ ] Logging and alerting configured

---

## Environment Setup

### Prerequisites

- Rust 1.70+ with `wasm32-unknown-unknown` target
- Stellar CLI (latest version)
- Hardware wallet or multisig setup for admin keys
- Mainnet XLM for deployment fees (~10 XLM recommended)
- Access to monitoring infrastructure (Datadog, New Relic, etc.)

### Install Dependencies

```bash
# Update Rust
rustup update stable
rustup target add wasm32-unknown-unknown

# Install Stellar CLI
cargo install --locked stellar-cli

# Verify installations
rustc --version
stellar --version
```

### Network Configuration

```bash
# Add mainnet network
stellar network add mainnet \
  --rpc-url https://rpc.mainnet.stellar.org:443 \
  --network-passphrase "Public Global Stellar Network ; September 2015"

# Verify configuration
stellar network list
stellar network use mainnet
```

### Environment Variables

Create `.env.mainnet` (never commit to version control):

```bash
# Network
NETWORK=mainnet
RPC_URL=https://rpc.mainnet.stellar.org:443

# Deployment
DEPLOYER_SECRET_KEY="S..."          # Hardware wallet or multisig signer
DEPLOYER_ADDRESS="GB..."            # Deployer public key

# Admin Configuration
ADMIN_ADDRESSES="GB...,GB...,GB..."  # Comma-separated admin addresses
ADMIN_THRESHOLD=2                    # Minimum signatures required (2-of-3 recommended)

# Token Configuration
TOKEN_CONTRACT="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"  # XLM token on mainnet

# Monitoring
SENTRY_DSN="https://..."             # Error tracking
DATADOG_API_KEY="..."                # Metrics and logs
SLACK_WEBHOOK_URL="https://..."      # Alerts

# Backup
BACKUP_STORAGE_URL="s3://..."        # S3 or similar
BACKUP_ENCRYPTION_KEY="..."          # Encryption key for backups
```

Add to `.gitignore`:

```bash
.env.mainnet
.env.*.local
*.key
*.pem
```

---

## Contract Deployment

### Step 1: Build Optimized WASM

```bash
cd QuorumCredit

# Clean previous builds
cargo clean

# Build optimized WASM
cargo build --target wasm32-unknown-unknown --release

# Verify build
ls -lh target/wasm32-unknown-unknown/release/quorum_credit.wasm

# Expected size: ~200-300 KB
```

### Step 2: Deploy Contract

```bash
# Load environment
source .env.mainnet

# Deploy contract (note the returned CONTRACT_ID)
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/quorum_credit.wasm \
  --network mainnet \
  --source $DEPLOYER_SECRET_KEY | grep -oP 'Contract ID: \K.*')

echo "Contract deployed: $CONTRACT_ID"

# Save for later use
echo "CONTRACT_ID=$CONTRACT_ID" >> .env.mainnet
```

### Step 3: Initialize Contract

Initialize immediately after deployment using the same deployer key:

```bash
# Parse admin addresses
IFS=',' read -ra ADMINS <<< "$ADMIN_ADDRESSES"

# Initialize contract
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn initialize \
  --network mainnet \
  --source $DEPLOYER_SECRET_KEY \
  -- \
  --deployer $DEPLOYER_ADDRESS \
  --admins "[$(printf '"%s",' "${ADMINS[@]}" | sed 's/,$/')]" \
  --admin_threshold $ADMIN_THRESHOLD \
  --token $TOKEN_CONTRACT

echo "Contract initialized successfully"
```

### Step 4: Verify Deployment

```bash
# Check contract exists
stellar contract info \
  --id $CONTRACT_ID \
  --network mainnet

# Get configuration
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet

# Verify admins
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admins \
  --network mainnet
```

---

## Post-Deployment Verification

### Smoke Tests

Run basic functionality tests on mainnet:

```bash
#!/bin/bash
set -e

CONTRACT_ID=$1
NETWORK=mainnet

echo "Running smoke tests..."

# Test 1: Get config
echo "✓ Testing get_config..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network $NETWORK

# Test 2: Get admins
echo "✓ Testing get_admins..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admins \
  --network $NETWORK

# Test 3: Check fee treasury
echo "✓ Testing get_fee_treasury..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_fee_treasury \
  --network $NETWORK

echo "✓ All smoke tests passed"
```

### Contract Verification

```bash
# Verify contract code hash
stellar contract info --id $CONTRACT_ID --network mainnet

# Compare with local build
sha256sum target/wasm32-unknown-unknown/release/quorum_credit.wasm

# Document in deployment log
```

---

## Operational Procedures

### Daily Operations

#### Health Check

```bash
#!/bin/bash
# Run daily health checks

CONTRACT_ID=$1

echo "=== QuorumCredit Health Check ==="
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Check contract is responsive
if stellar contract invoke --id $CONTRACT_ID --fn get_config --network mainnet > /dev/null 2>&1; then
  echo "✓ Contract responsive"
else
  echo "✗ Contract not responding"
  exit 1
fi

# Check fee treasury
FEE_TREASURY=$(stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_fee_treasury \
  --network mainnet)
echo "Fee Treasury: $FEE_TREASURY stroops"

# Check admin configuration
ADMINS=$(stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admins \
  --network mainnet)
echo "Admins: $ADMINS"

echo "✓ Health check complete"
```

#### Backup Procedures

```bash
#!/bin/bash
# Daily backup of contract state

CONTRACT_ID=$1
BACKUP_DIR="./backups/$(date +%Y%m%d)"
mkdir -p $BACKUP_DIR

echo "Backing up contract state..."

# Export configuration
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet > $BACKUP_DIR/config.json

# Export admins
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admins \
  --network mainnet > $BACKUP_DIR/admins.json

# Encrypt and upload
tar czf - $BACKUP_DIR | \
  openssl enc -aes-256-cbc -salt -pass env:BACKUP_ENCRYPTION_KEY | \
  aws s3 cp - s3://quorum-credit-backups/$(date +%Y%m%d).tar.gz.enc

echo "✓ Backup complete"
```

### Admin Operations

#### Pause Contract (Emergency)

```bash
#!/bin/bash
# Pause contract in case of emergency

CONTRACT_ID=$1
ADMIN_SIGNERS=$2  # Comma-separated admin addresses

stellar contract invoke \
  --id $CONTRACT_ID \
  --fn pause \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]"

echo "✓ Contract paused"
```

#### Unpause Contract

```bash
#!/bin/bash
# Resume contract operations

CONTRACT_ID=$1
ADMIN_SIGNERS=$2

stellar contract invoke \
  --id $CONTRACT_ID \
  --fn unpause \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]"

echo "✓ Contract unpaused"
```

#### Update Configuration

```bash
#!/bin/bash
# Update protocol configuration

CONTRACT_ID=$1
ADMIN_SIGNERS=$2
YIELD_BPS=$3      # New yield rate in basis points
SLASH_BPS=$4      # New slash rate in basis points

stellar contract invoke \
  --id $CONTRACT_ID \
  --fn update_config \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]" \
  --yield_bps $YIELD_BPS \
  --slash_bps $SLASH_BPS

echo "✓ Configuration updated"
```

---

## Monitoring & Alerting

### Metrics to Monitor

1. **Contract Health**
   - Response time (< 5s)
   - Error rate (< 0.1%)
   - Transaction success rate (> 99%)

2. **Financial Metrics**
   - Total vouched amount
   - Total loans disbursed
   - Total repayments received
   - Fee treasury balance

3. **User Activity**
   - Active borrowers
   - Active vouchers
   - Loan default rate
   - Average loan size

### Datadog Integration

```python
# monitoring/datadog_client.py
from datadog import initialize, api
import os

options = {
    'api_key': os.getenv('DATADOG_API_KEY'),
    'app_key': os.getenv('DATADOG_APP_KEY')
}

initialize(**options)

def send_metric(metric_name, value, tags=None):
    """Send metric to Datadog"""
    api.Metric.send(
        metric=f"quorum_credit.{metric_name}",
        points=value,
        tags=tags or []
    )

def send_event(title, text, alert_type='info'):
    """Send event to Datadog"""
    api.Event.create(
        title=title,
        text=text,
        alert_type=alert_type,
        tags=['quorum_credit']
    )
```

### Alert Rules

```yaml
# alerts.yaml
alerts:
  - name: contract_unresponsive
    condition: "avg:quorum_credit.response_time{*} > 5000"
    message: "QuorumCredit contract not responding"
    severity: critical

  - name: high_error_rate
    condition: "avg:quorum_credit.error_rate{*} > 0.001"
    message: "QuorumCredit error rate exceeds 0.1%"
    severity: high

  - name: low_fee_treasury
    condition: "avg:quorum_credit.fee_treasury{*} < 1000000"
    message: "Fee treasury balance low"
    severity: medium

  - name: high_default_rate
    condition: "avg:quorum_credit.default_rate{*} > 0.05"
    message: "Loan default rate exceeds 5%"
    severity: high
```

---

## Incident Response

### Incident Classification

| Severity | Response Time | Example |
|----------|---------------|---------|
| Critical | Immediate | Contract compromised, funds at risk |
| High | 1 hour | High error rate, data corruption |
| Medium | 4 hours | Performance degradation |
| Low | 24 hours | Minor bugs, documentation issues |

### Response Procedures

#### Critical Incident

1. **Immediate Actions** (0-5 minutes)
   - Pause contract: `./scripts/pause_contract.sh`
   - Notify team via Slack
   - Create incident ticket

2. **Investigation** (5-30 minutes)
   - Analyze logs and metrics
   - Identify root cause
   - Assess impact

3. **Mitigation** (30+ minutes)
   - Deploy fix or rollback
   - Verify fix on testnet first
   - Unpause contract
   - Monitor closely

4. **Post-Incident** (24 hours)
   - Document incident
   - Conduct post-mortem
   - Implement preventive measures

### Rollback Procedure

```bash
#!/bin/bash
# Rollback to previous contract version

CONTRACT_ID=$1
PREVIOUS_WASM_HASH=$2
ADMIN_SIGNERS=$3

# Step 1: Pause contract
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn pause \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]"

# Step 2: Upgrade to previous version
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn upgrade \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]" \
  --new_wasm_hash $PREVIOUS_WASM_HASH

# Step 3: Unpause
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn unpause \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]"

echo "✓ Rollback complete"
```

---

## Upgrade Procedures

### Planning an Upgrade

1. **Preparation** (1-2 weeks before)
   - Code review and testing
   - Testnet deployment and validation
   - Security audit if significant changes
   - Communication to users

2. **Pre-Upgrade** (24 hours before)
   - Final testing on testnet
   - Prepare rollback plan
   - Notify stakeholders
   - Schedule maintenance window

### Upgrade Steps

```bash
#!/bin/bash
# Upgrade contract to new version

CONTRACT_ID=$1
ADMIN_SIGNERS=$2

echo "Starting contract upgrade..."

# Step 1: Build new WASM
echo "Building new WASM..."
cargo build --target wasm32-unknown-unknown --release

# Step 2: Pause contract
echo "Pausing contract..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn pause \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]"

# Step 3: Install new WASM
echo "Installing new WASM..."
NEW_WASM_HASH=$(stellar contract install \
  --wasm target/wasm32-unknown-unknown/release/quorum_credit.wasm \
  --network mainnet \
  --source $ADMIN_SECRET_KEY)

echo "New WASM hash: $NEW_WASM_HASH"

# Step 4: Upgrade contract
echo "Upgrading contract..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn upgrade \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]" \
  --new_wasm_hash $NEW_WASM_HASH

# Step 5: Verify upgrade
echo "Verifying upgrade..."
stellar contract info --id $CONTRACT_ID --network mainnet

# Step 6: Unpause contract
echo "Unpausing contract..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn unpause \
  --network mainnet \
  --source $ADMIN_SECRET_KEY \
  -- \
  --admin_signers "[$ADMIN_SIGNERS]"

echo "✓ Upgrade complete"
```

### Post-Upgrade Verification

```bash
#!/bin/bash
# Verify upgrade was successful

CONTRACT_ID=$1

echo "=== Post-Upgrade Verification ==="

# Check contract is responsive
echo "Checking contract responsiveness..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_config \
  --network mainnet

# Verify state integrity
echo "Verifying state integrity..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_admins \
  --network mainnet

# Check fee treasury
echo "Checking fee treasury..."
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn get_fee_treasury \
  --network mainnet

echo "✓ Post-upgrade verification complete"
```

---

## Security Best Practices

### Key Management

- Use hardware wallets for admin keys
- Implement 2-of-3 or 3-of-5 multisig for critical operations
- Rotate keys annually
- Never commit secret keys to version control
- Use environment variables or secure vaults

### Access Control

- Limit admin access to authorized personnel only
- Implement role-based access control (RBAC)
- Audit all admin operations
- Require approval for sensitive operations

### Monitoring & Logging

- Enable comprehensive logging
- Monitor for suspicious activity
- Set up alerts for anomalies
- Retain logs for at least 90 days
- Encrypt logs in transit and at rest

### Incident Response

- Maintain incident response plan
- Conduct regular drills
- Document all incidents
- Perform post-mortems
- Implement preventive measures

---

## Support & Resources

- [Stellar Documentation](https://developers.stellar.org)
- [Soroban Docs](https://soroban.stellar.org)
- [QuorumCredit GitHub](https://github.com/QuorumCredit/QuorumCredit)
- [Stellar Developer Discord](https://discord.gg/stellardev)

For issues or questions, open an issue on GitHub or contact the team.
