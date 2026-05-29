# QuorumCredit Troubleshooting Guide

This guide covers common issues and their solutions for QuorumCredit operators, developers, and users.

## Table of Contents

- [Deployment Issues](#deployment-issues)
- [Contract Interaction Issues](#contract-interaction-issues)
- [Loan & Vouching Issues](#loan--vouching-issues)
- [Performance & Monitoring](#performance--monitoring)
- [Error Codes & Resolution](#error-codes--resolution)

---

## Deployment Issues

### Contract Deployment Fails

**Symptom**: `stellar contract deploy` command fails with compilation or network errors.

**Solutions**:
1. **Verify Rust toolchain**: Ensure you have the correct Rust version and wasm target:
   ```bash
   rustup update
   rustup target add wasm32-unknown-unknown
   ```

2. **Check network connectivity**: Verify your RPC endpoint is reachable:
   ```bash
   curl https://soroban-testnet.stellar.org:443/health
   ```

3. **Validate WASM build**: Build locally first:
   ```bash
   cd QuorumCredit
   cargo build --target wasm32-unknown-unknown --release
   ```

4. **Check account balance**: Ensure your deployer account has sufficient XLM for fees:
   ```bash
   stellar account info --source $DEPLOYER_SECRET_KEY --network testnet
   ```

### Initialize Fails After Deployment

**Symptom**: `initialize` call panics with "unauthorized" or "require_auth failed".

**Solutions**:
1. **Verify deployer signature**: The `--source` key in the `invoke` command must match the `--deployer` parameter:
   ```bash
   # CORRECT: Same key for both deploy and initialize
   stellar contract deploy --source $DEPLOYER_SECRET_KEY ...
   stellar contract invoke --source $DEPLOYER_SECRET_KEY --fn initialize -- --deployer $DEPLOYER_ADDRESS ...
   ```

2. **Check deployer address**: Ensure `$DEPLOYER_ADDRESS` matches the public key of `$DEPLOYER_SECRET_KEY`:
   ```bash
   stellar keys list  # Verify your keys
   ```

3. **Timing issue**: If initialize is called too long after deploy, the contract state may have changed. Re-deploy and initialize immediately.

### Contract Upgrade Fails

**Symptom**: `upgrade` function returns error or panics.

**Solutions**:
1. **Verify admin threshold**: Ensure you have enough admin signatures:
   ```bash
   stellar contract invoke --id $CONTRACT_ID --fn get_admins --network testnet
   ```

2. **Check WASM hash**: Verify the new WASM hash is correct:
   ```bash
   stellar contract install --wasm target/wasm32-unknown-unknown/release/quorum_credit.wasm --network testnet
   ```

3. **Pause before upgrade**: Recommended to pause the contract first:
   ```bash
   stellar contract invoke --id $CONTRACT_ID --fn pause --network testnet --source $ADMIN_SECRET_KEY
   ```

---

## Contract Interaction Issues

### Transaction Timeout

**Symptom**: Transaction hangs or times out after submission.

**Solutions**:
1. **Increase timeout**: Use `--timeout` flag:
   ```bash
   stellar contract invoke --timeout 300 ...
   ```

2. **Check network congestion**: Monitor Soroban network status at https://status.stellar.org

3. **Verify transaction status**: Check if transaction was submitted:
   ```bash
   stellar tx info $TRANSACTION_HASH --network testnet
   ```

### Insufficient Funds Error

**Symptom**: `InsufficientFunds` error when calling contract functions.

**Solutions**:
1. **Check contract balance**: Verify the contract holds enough tokens:
   ```bash
   stellar contract invoke --id $CONTRACT_ID --fn get_contract_balance --network testnet
   ```

2. **Check account balance**: Ensure your account has XLM for transaction fees:
   ```bash
   stellar account info --source $YOUR_SECRET_KEY --network testnet
   ```

3. **Verify token allowance**: If using SEP-41 tokens, check approval:
   ```bash
   stellar contract invoke --id $TOKEN_CONTRACT --fn allowance --network testnet -- \
     --from $YOUR_ADDRESS --spender $CONTRACT_ID
   ```

### Authorization Failures

**Symptom**: `UnauthorizedCaller` or `require_auth` errors.

**Solutions**:
1. **Verify transaction signer**: Ensure the correct account is signing:
   ```bash
   # Check which key is being used
   stellar keys list
   ```

2. **Check role requirements**: Verify your account has the required role:
   - **Voucher**: Must sign `vouch()` calls
   - **Borrower**: Must sign `request_loan()` and `repay()` calls
   - **Admin**: Must sign admin functions

3. **Multi-sig verification**: For admin functions, ensure all required signers are present:
   ```bash
   stellar contract invoke --id $CONTRACT_ID --fn get_admins --network testnet
   ```

---

## Loan & Vouching Issues

### Loan Request Rejected

**Symptom**: `request_loan` fails with error code.

**Solutions**:

| Error | Cause | Fix |
|-------|-------|-----|
| `InsufficientFunds` | Total vouched stake < threshold | Recruit more vouchers or lower threshold |
| `LoanBelowMinAmount` | Requested amount too small | Request at least `get_config().min_loan_amount` |
| `LoanExceedsMaxAmount` | Requested amount too large | Request smaller amount or admin raises cap |
| `InsufficientVouchers` | Too few vouchers | Recruit more vouchers to meet minimum |
| `Blacklisted` | Borrower is blacklisted | Contact admin to remove from blacklist |
| `VouchTooRecent` | Vouches added too recently | Wait for vouch age requirement to pass |

### Vouch Fails

**Symptom**: `vouch` call fails.

**Solutions**:

| Error | Cause | Fix |
|-------|-------|-----|
| `MinStakeNotMet` | Stake below minimum | Increase stake to at least 50 stroops |
| `DuplicateVouch` | Already vouching for this borrower | Use `increase_stake()` instead |
| `ActiveLoanExists` | Borrower has active loan | Wait for loan to be repaid or slashed |
| `InvalidToken` | Token not allowed | Use primary token or admin-approved token |
| `InsufficientVoucherBalance` | Voucher lacks tokens | Transfer tokens to voucher account |

### Repayment Issues

**Symptom**: `repay` call fails or doesn't distribute yield.

**Solutions**:

| Error | Cause | Fix |
|-------|-------|-----|
| `NoActiveLoan` | No active loan for borrower | Verify borrower address and loan status |
| `LoanPastDeadline` | Loan deadline passed | Use `slash()` to mark default |
| `UnauthorizedCaller` | Wrong account calling repay | Ensure borrower signs the transaction |
| `InsufficientFunds` | Contract lacks yield funds | Admin must fund yield reserve |

**Yield not received**:
- Verify vouch stake ≥ 50 stroops (minimum for non-zero yield)
- Check contract has sufficient XLM for yield distribution
- Confirm loan was fully repaid

---

## Performance & Monitoring

### High Transaction Fees

**Symptom**: Transaction fees are unexpectedly high.

**Solutions**:
1. **Use testnet for testing**: Testnet has lower fees than mainnet
2. **Batch operations**: Use `batch_vouch()` to reduce transaction count
3. **Monitor network**: Check Soroban network load at https://status.stellar.org

### Slow Contract Responses

**Symptom**: Contract queries take a long time.

**Solutions**:
1. **Check RPC endpoint**: Verify your RPC node is responsive:
   ```bash
   curl -X POST https://soroban-testnet.stellar.org:443 \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
   ```

2. **Use pagination**: For large datasets, use pagination helpers:
   ```bash
   stellar contract invoke --id $CONTRACT_ID --fn get_vouches_paginated \
     --network testnet -- --borrower $BORROWER --offset 0 --limit 10
   ```

3. **Cache results**: Store frequently accessed data locally to reduce RPC calls

### Monitoring Contract Health

**Check contract status**:
```bash
# Get configuration
stellar contract invoke --id $CONTRACT_ID --fn get_config --network testnet

# Get admin list
stellar contract invoke --id $CONTRACT_ID --fn get_admins --network testnet

# Check pause state
stellar contract invoke --id $CONTRACT_ID --fn is_paused --network testnet

# Get fee treasury balance
stellar contract invoke --id $CONTRACT_ID --fn get_fee_treasury --network testnet
```

---

## Error Codes & Resolution

For detailed error code reference, see [Error Reference](../README.md#error-reference).

### Quick Error Resolution

**Error 1 - InsufficientFunds**
- Ensure positive amounts
- Verify contract balance for loans
- Check total stake meets threshold

**Error 2 - ActiveLoanExists**
- Wait for existing loan to be repaid or slashed
- Cannot add new vouches during active loan

**Error 3 - StakeOverflow**
- Reduce number or size of vouches
- Split borrower across multiple accounts if needed

**Error 6 - NoActiveLoan**
- Verify borrower address
- Confirm loan has been disbursed
- Check loan hasn't already been closed

**Error 7 - ContractPaused**
- Wait for admin to unpause
- Contact protocol administrators

**Error 8 - LoanPastDeadline**
- Loans must be repaid before deadline
- Use slash for defaults after deadline

**Error 13 - MinStakeNotMet**
- Increase stake to minimum required
- Check `get_config().min_stake`

**Error 24 - Blacklisted**
- Contact protocol admin
- Provide reason for blacklist removal

---

## Getting Help

### Resources

- **Documentation**: https://github.com/QuorumCredit/QuorumCredit/tree/main/docs
- **API Reference**: See [README.md](../README.md#api-reference)
- **Security Issues**: See [SECURITY.md](../SECURITY.md)
- **Contributing**: See [CONTRIBUTING.md](../CONTRIBUTING.md)

### Reporting Issues

1. **Check existing issues**: https://github.com/QuorumCredit/QuorumCredit/issues
2. **Provide details**:
   - Error message and code
   - Steps to reproduce
   - Network (testnet/mainnet)
   - Contract version
3. **Security vulnerabilities**: Report privately via GitHub Security Advisories

### Community Support

- **Discord**: [Stellar Developer Discord](https://discord.gg/stellardev)
- **GitHub Discussions**: https://github.com/QuorumCredit/QuorumCredit/discussions
- **Email**: security@quorumcredit.dev

---

## Appendix: Common Commands

```bash
# Check contract configuration
stellar contract invoke --id $CONTRACT_ID --fn get_config --network testnet

# Get loan status
stellar contract invoke --id $CONTRACT_ID --fn loan_status --network testnet -- --borrower $BORROWER

# Get vouches for borrower
stellar contract invoke --id $CONTRACT_ID --fn get_vouches --network testnet -- --borrower $BORROWER

# Check eligibility
stellar contract invoke --id $CONTRACT_ID --fn is_eligible --network testnet -- \
  --borrower $BORROWER --threshold 1000000000 --token_addr $TOKEN

# Get total vouched amount
stellar contract invoke --id $CONTRACT_ID --fn total_vouched --network testnet -- --borrower $BORROWER

# Check if contract is paused
stellar contract invoke --id $CONTRACT_ID --fn is_paused --network testnet
```
