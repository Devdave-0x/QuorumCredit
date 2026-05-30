# QuorumCredit Security Best Practices

This guide provides security best practices for operators, developers, and users of QuorumCredit.

## Table of Contents

- [Operator Security](#operator-security)
- [Key Management](#key-management)
- [Contract Deployment](#contract-deployment)
- [Access Control](#access-control)
- [Monitoring & Incident Response](#monitoring--incident-response)
- [User Security](#user-security)
- [Development Security](#development-security)

---

## Operator Security

### Admin Key Management

**Critical**: Admin keys control the entire protocol. Compromise of admin keys can lead to:
- Unauthorized contract upgrades
- Configuration changes
- Fund theft
- Protocol shutdown

**Best Practices**:

1. **Use Hardware Wallets**: Store admin keys on hardware wallets (Ledger, Trezor)
   ```bash
   # Never store keys in plaintext files
   # Use hardware wallet for signing
   stellar keys add admin-hw --hw
   ```

2. **Implement Multisig**: Require multiple signatures for admin functions
   ```bash
   # Initialize with 3-of-5 multisig
   stellar contract invoke --id $CONTRACT_ID --fn initialize \
     --source $DEPLOYER_SECRET_KEY -- \
     --admins '["'$ADMIN1'","'$ADMIN2'","'$ADMIN3'","'$ADMIN4'","'$ADMIN5'"]' \
     --admin_threshold 3
   ```

3. **Separate Roles**: Use different keys for different functions
   - **Deployer key**: Only for initial deployment and initialization
   - **Admin keys**: For ongoing governance
   - **Operator key**: For routine operations (pause/unpause)

4. **Rotate Keys Regularly**: Change admin keys periodically
   - Quarterly minimum
   - Immediately if compromise is suspected
   - After staff changes

5. **Secure Key Storage**:
   - Never commit keys to version control
   - Use `.env` files with `.gitignore`
   - Store in secure vaults (HashiCorp Vault, AWS Secrets Manager)
   - Encrypt at rest

### Admin Threshold Configuration

**Recommendation**: Use at least 2-of-3 multisig for production

| Scenario | Threshold | Rationale |
|----------|-----------|-----------|
| Testnet | 1-of-1 | Development only |
| Staging | 2-of-3 | Prevent single-key compromise |
| Mainnet | 3-of-5 | High security, operational flexibility |

### Pause Mechanism

**Use pause for**:
- Emergency response to security issues
- Contract upgrades
- Maintenance windows
- Investigating anomalies

**Pause procedure**:
```bash
# 1. Pause the contract
stellar contract invoke --id $CONTRACT_ID --fn pause --network mainnet \
  --source $ADMIN_SECRET_KEY -- --admin_signers '["'$ADMIN1'","'$ADMIN2'","'$ADMIN3'"]'

# 2. Investigate and fix
# ... perform investigation ...

# 3. Unpause when ready
stellar contract invoke --id $CONTRACT_ID --fn unpause --network mainnet \
  --source $ADMIN_SECRET_KEY -- --admin_signers '["'$ADMIN1'","'$ADMIN2'","'$ADMIN3'"]'
```

---

## Key Management

### Secret Key Security

**Never**:
- Commit secret keys to version control
- Share keys via email or chat
- Store keys in plaintext
- Use the same key for multiple purposes
- Reuse keys across networks (testnet/mainnet)

**Always**:
- Use environment variables for keys
- Rotate keys regularly
- Use hardware wallets for production
- Implement key access logging
- Backup keys securely

### Environment Variables

**Secure `.env` setup**:
```bash
# .env (NEVER commit this)
NETWORK=mainnet
DEPLOYER_SECRET_KEY="SB..."
ADMIN_SECRET_KEY_1="SB..."
ADMIN_SECRET_KEY_2="SB..."
ADMIN_SECRET_KEY_3="SB..."
TOKEN_CONTRACT="CA..."
```

**Protect `.env`**:
```bash
# Add to .gitignore
echo ".env" >> .gitignore
echo ".env.local" >> .gitignore

# Restrict file permissions
chmod 600 .env

# Use secure vaults in production
# Example: AWS Secrets Manager
aws secretsmanager get-secret-value --secret-id quorum-credit-keys
```

### Key Rotation

**Rotation schedule**:
- **Quarterly**: Routine rotation
- **Immediately**: If compromise suspected
- **After staff changes**: Remove departing team member's keys
- **After security incident**: Rotate all keys

**Rotation procedure**:
1. Generate new admin keys
2. Update contract configuration with new keys
3. Verify new keys work
4. Revoke old keys
5. Document rotation in audit log

---

## Contract Deployment

### Pre-Deployment Checklist

- [ ] All tests passing: `cargo test`
- [ ] No clippy warnings: `cargo clippy`
- [ ] Code reviewed by 2+ team members
- [ ] Security audit completed (for mainnet)
- [ ] Testnet deployment verified
- [ ] Deployment script tested
- [ ] Rollback plan documented
- [ ] Monitoring configured
- [ ] Communication plan ready

### Deployment Sequence

**Critical**: Follow this exact sequence to prevent front-running attacks

```bash
# Step 1: Build WASM
cargo build --target wasm32-unknown-unknown --release

# Step 2: Deploy contract (deployer signs)
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/quorum_credit.wasm \
  --network mainnet \
  --source $DEPLOYER_SECRET_KEY)

# Step 3: Initialize immediately (SAME deployer key)
stellar contract invoke \
  --id $CONTRACT_ID \
  --fn initialize \
  --network mainnet \
  --source $DEPLOYER_SECRET_KEY \
  -- \
  --deployer $DEPLOYER_ADDRESS \
  --admins '["'$ADMIN1'","'$ADMIN2'","'$ADMIN3'"]' \
  --admin_threshold 2 \
  --token $TOKEN_CONTRACT
```

**Why this matters**: If initialization is delayed, an attacker could call `initialize` first with malicious parameters.

### Testnet Verification

Before mainnet deployment:

1. **Deploy to testnet**: Follow deployment sequence
2. **Run integration tests**: Test all critical paths
3. **Verify configuration**: Check `get_config()` returns expected values
4. **Test admin functions**: Verify multisig works
5. **Simulate incident**: Test pause/unpause
6. **Monitor for 24 hours**: Watch for anomalies

### Upgrade Safety

**Before upgrading**:
1. Pause the contract
2. Backup current state (if possible)
3. Test new WASM on testnet
4. Verify upgrade procedure
5. Prepare rollback plan

**Upgrade procedure**:
```bash
# 1. Build new WASM
cargo build --target wasm32-unknown-unknown --release

# 2. Pause contract
stellar contract invoke --id $CONTRACT_ID --fn pause --network mainnet \
  --source $ADMIN_SECRET_KEY -- --admin_signers '["'$ADMIN1'","'$ADMIN2'"]'

# 3. Install new WASM
NEW_WASM_HASH=$(stellar contract install \
  --wasm target/wasm32-unknown-unknown/release/quorum_credit.wasm \
  --network mainnet \
  --source $ADMIN_SECRET_KEY)

# 4. Upgrade contract
stellar contract invoke --id $CONTRACT_ID --fn upgrade --network mainnet \
  --source $ADMIN_SECRET_KEY -- \
  --admin_signers '["'$ADMIN1'","'$ADMIN2'"]' \
  --new_wasm_hash $NEW_WASM_HASH

# 5. Unpause contract
stellar contract invoke --id $CONTRACT_ID --fn unpause --network mainnet \
  --source $ADMIN_SECRET_KEY -- --admin_signers '["'$ADMIN1'","'$ADMIN2'"]'
```

---

## Access Control

### Role-Based Access

| Role | Functions | Requirements |
|------|-----------|--------------|
| **Deployer** | `initialize` | Must sign deployment tx |
| **Admin** | `pause`, `unpause`, `upgrade`, `set_config` | Must meet `admin_threshold` |
| **Voucher** | `vouch`, `increase_stake`, `decrease_stake`, `withdraw_vouch` | Must sign tx |
| **Borrower** | `request_loan`, `repay` | Must sign tx |
| **Anyone** | `get_config`, `get_loan`, `get_vouches`, `is_eligible` | Read-only |

### Authorization Checks

**Always verify**:
- Caller is authorized for the function
- Caller has required signatures (for multisig)
- Caller has sufficient balance (for token transfers)
- Caller is not blacklisted (for borrowers)

**Example**:
```rust
// Verify caller is the borrower
borrower.require_auth();

// Verify caller is an admin (multisig)
verify_admin_signatures(&env, &admin_signers, &admin_threshold)?;
```

### Blacklist Management

**Use blacklist for**:
- Repeat defaulters
- Fraudulent borrowers
- Sanctioned addresses
- Compromised accounts

**Blacklist procedure**:
```bash
# Add to blacklist
stellar contract invoke --id $CONTRACT_ID --fn add_to_blacklist \
  --network mainnet --source $ADMIN_SECRET_KEY -- \
  --admin_signers '["'$ADMIN1'","'$ADMIN2'"]' \
  --borrower $BORROWER_ADDRESS

# Remove from blacklist
stellar contract invoke --id $CONTRACT_ID --fn remove_from_blacklist \
  --network mainnet --source $ADMIN_SECRET_KEY -- \
  --admin_signers '["'$ADMIN1'","'$ADMIN2'"]' \
  --borrower $BORROWER_ADDRESS
```

---

## Monitoring & Incident Response

### Monitoring Setup

**Monitor these metrics**:
- Contract balance (should never go negative)
- Loan disbursements (unusual spikes)
- Default rate (should be < 5%)
- Yield distribution (should match calculations)
- Admin actions (all should be logged)
- Contract pause state (should be unpaused normally)

**Monitoring tools**:
- Stellar Horizon API for transaction history
- Soroban RPC for contract state
- Custom indexer for event tracking
- Alerting system (PagerDuty, Opsgenie)

### Incident Response Plan

**Incident severity levels**:

| Level | Impact | Response Time |
|-------|--------|----------------|
| **Critical** | Funds at risk, contract compromised | Immediate (< 5 min) |
| **High** | Significant functionality broken | 15 minutes |
| **Medium** | Minor functionality broken | 1 hour |
| **Low** | Documentation or UI issues | 24 hours |

**Critical incident response**:
1. **Pause contract** (< 1 minute)
2. **Notify stakeholders** (< 5 minutes)
3. **Investigate** (ongoing)
4. **Develop fix** (parallel)
5. **Test fix on testnet** (before deploying)
6. **Deploy fix** (multisig approval)
7. **Unpause contract** (after verification)
8. **Post-mortem** (within 24 hours)

### Logging & Auditing

**Log all**:
- Admin actions (pause, unpause, upgrade, config changes)
- Loan disbursements and repayments
- Slash events
- Authorization failures
- Configuration changes

**Audit trail**:
```bash
# Query contract events
stellar events --id $CONTRACT_ID --network mainnet

# Filter by event type
stellar events --id $CONTRACT_ID --network mainnet --type "admin/*"

# Export for analysis
stellar events --id $CONTRACT_ID --network mainnet --format json > audit.json
```

### Disaster Recovery

**Backup procedures**:
1. **Contract state**: Snapshot contract storage regularly
2. **Configuration**: Backup `get_config()` output
3. **Admin keys**: Secure backup of admin keys (encrypted)
4. **Documentation**: Keep deployment and configuration docs updated

**Recovery procedures**:
1. **Identify issue**: Determine what went wrong
2. **Pause contract**: Stop all operations
3. **Assess damage**: Determine scope of impact
4. **Develop fix**: Create patched WASM
5. **Deploy fix**: Use upgrade procedure
6. **Verify recovery**: Confirm state is correct
7. **Unpause**: Resume operations

---

## User Security

### For Borrowers

**Protect your account**:
- Use hardware wallet for loan requests
- Never share your secret key
- Verify contract address before interacting
- Check loan terms before accepting
- Set calendar reminders for repayment deadlines

**Repayment security**:
- Repay before deadline to avoid default
- Verify repayment amount before sending
- Keep proof of repayment (transaction hash)
- Confirm yield was received

### For Vouchers

**Protect your stake**:
- Use hardware wallet for vouching
- Never share your secret key
- Verify borrower identity before vouching
- Start with small stakes to test
- Diversify across multiple borrowers

**Vouch responsibly**:
- Only vouch for people you trust
- Understand the risks (50% slash on default)
- Monitor borrower's loan status
- Participate in slash votes
- Withdraw vouches when no longer comfortable

### Phishing Prevention

**Be aware of**:
- Fake contract addresses
- Phishing emails claiming to be from QuorumCredit
- Fake websites mimicking QuorumCredit
- Social engineering attacks

**Verify authenticity**:
- Check contract address on GitHub
- Use official website only
- Verify email sender domain
- Never click links in unsolicited emails
- Use hardware wallet for all transactions

---

## Development Security

### Code Review

**All code changes require**:
- 2+ peer reviews
- Security review for sensitive code
- Automated testing (100% coverage for critical paths)
- Clippy checks (no warnings)
- Cargo audit (no vulnerabilities)

### Dependency Management

**Secure dependencies**:
```bash
# Check for vulnerabilities
cargo audit

# Update dependencies
cargo update

# Pin versions in Cargo.toml
soroban-sdk = "=20.0.0"  # Exact version
```

**Avoid**:
- Unvetted dependencies
- Dependencies with known vulnerabilities
- Outdated dependencies
- Typosquatting variants

### Testing

**Test coverage**:
- Unit tests: 100% for critical functions
- Integration tests: All user flows
- Property-based tests: Invariants
- Fuzz tests: Edge cases
- Security tests: Authorization, overflow, underflow

**Test execution**:
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_vouch_and_loan_disbursed

# Generate coverage
cargo tarpaulin --out Html
```

### Security Audit

**Before mainnet deployment**:
1. **Internal audit**: Team review
2. **External audit**: Third-party security firm
3. **Formal verification**: Mathematical proof of correctness (optional)
4. **Bug bounty**: Incentivize community to find issues

**Audit checklist**:
- [ ] No integer overflow/underflow
- [ ] No reentrancy vulnerabilities
- [ ] Proper authorization checks
- [ ] Correct yield/slash calculations
- [ ] State consistency maintained
- [ ] Error handling complete
- [ ] No hardcoded values
- [ ] Proper event logging

---

## Vulnerability Disclosure

### Reporting Security Issues

**Do not**:
- Open public GitHub issues
- Post on social media
- Share with unauthorized parties
- Attempt to exploit vulnerabilities

**Do**:
- Report privately via [GitHub Security Advisories](https://github.com/QuorumCredit/QuorumCredit/security/advisories/new)
- Include detailed reproduction steps
- Allow time for fix before disclosure
- Follow responsible disclosure timeline

### Responsible Disclosure Timeline

1. **Day 0**: Report vulnerability
2. **Day 1**: Acknowledgment from team
3. **Day 7**: Initial assessment
4. **Day 30**: Fix developed and tested
5. **Day 45**: Fix deployed to mainnet
6. **Day 60**: Public disclosure (if appropriate)

---

## Security Checklist

### Pre-Deployment

- [ ] All tests passing
- [ ] No clippy warnings
- [ ] No cargo audit vulnerabilities
- [ ] Code reviewed by 2+ team members
- [ ] Security audit completed
- [ ] Testnet deployment verified
- [ ] Admin multisig configured
- [ ] Monitoring configured
- [ ] Incident response plan ready
- [ ] Communication plan ready

### Post-Deployment

- [ ] Monitor contract balance
- [ ] Monitor loan metrics
- [ ] Monitor admin actions
- [ ] Review audit logs daily
- [ ] Rotate admin keys quarterly
- [ ] Update security documentation
- [ ] Conduct security training
- [ ] Test incident response procedures

### Ongoing

- [ ] Keep dependencies updated
- [ ] Monitor security advisories
- [ ] Conduct regular security reviews
- [ ] Perform penetration testing
- [ ] Update threat model
- [ ] Review and update this guide

---

## Resources

- [SECURITY.md](../SECURITY.md) - Vulnerability disclosure policy
- [Deployment Guide](../docs/deployment-guide.md) - Deployment procedures
- [Monitoring Guide](../docs/monitoring-guide.md) - Monitoring setup
- [Threat Model](../docs/threat-model.md) - Security threat analysis
- [Stellar Security](https://developers.stellar.org/docs/learn/security) - Stellar security best practices
- [Soroban Security](https://soroban.stellar.org/docs/learn/security) - Soroban security best practices

---

## Questions?

For security questions or concerns:
- Email: security@quorumcredit.dev
- GitHub: [Security Advisories](https://github.com/QuorumCredit/QuorumCredit/security/advisories)
- Discord: [Stellar Developer Discord](https://discord.gg/stellardev)
