# API Client Integration Guide

Complete guide for integrating with QuorumCredit using the TypeScript and Python client libraries.

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Authentication](#authentication)
4. [Core Operations](#core-operations)
5. [Error Handling](#error-handling)
6. [Advanced Usage](#advanced-usage)
7. [Examples](#examples)
8. [Troubleshooting](#troubleshooting)

---

## Installation

### TypeScript/JavaScript

```bash
# Using npm
npm install @quorum-credit/sdk

# Using yarn
yarn add @quorum-credit/sdk

# Using pnpm
pnpm add @quorum-credit/sdk
```

### Python

```bash
# Using pip
pip install quorum-credit

# Using poetry
poetry add quorum-credit

# Using pipenv
pipenv install quorum-credit
```

---

## Quick Start

### TypeScript

```typescript
import { QuorumCreditClient } from '@quorum-credit/sdk';
import { Keypair, Networks } from '@stellar/js-sdk';

// Initialize client
const keypair = Keypair.fromSecret('S...');
const client = new QuorumCreditClient({
  contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
  rpcUrl: 'https://soroban-testnet.stellar.org:443',
  networkPassphrase: Networks.TESTNET_NETWORK_PASSPHRASE,
  keypair,
});

// Vouch for a borrower
const txHash = await client.vouch({
  voucher: keypair.publicKey(),
  borrower: 'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
  stake: '1000000000', // 100 XLM in stroops
  token: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
});

console.log('Vouch transaction:', txHash);
```

### Python

```python
from quorum_credit import QuorumCreditClient, ClientConfig
from stellar_sdk import Keypair, Networks

# Initialize client
keypair = Keypair.from_secret('S...')
config = ClientConfig(
    contract_id='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
    rpc_url='https://soroban-testnet.stellar.org:443',
    network_passphrase=Networks.TESTNET_NETWORK_PASSPHRASE,
    keypair=keypair,
)
client = QuorumCreditClient(config)

# Vouch for a borrower
tx_hash = await client.vouch(
    voucher=keypair.public_key,
    borrower='GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    stake='1000000000',  # 100 XLM in stroops
    token='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
)

print(f'Vouch transaction: {tx_hash}')
```

---

## Authentication

### Keypair Management

All operations require a keypair to sign transactions. Never expose secret keys.

#### TypeScript

```typescript
import { Keypair } from '@stellar/js-sdk';

// Generate new keypair
const keypair = Keypair.random();
console.log('Public Key:', keypair.publicKey());
console.log('Secret Key:', keypair.secret()); // NEVER log in production

// Import from secret key
const keypair = Keypair.fromSecret('S...');

// Import from public key (read-only)
const publicKey = Keypair.fromPublicKey('GB...');
```

#### Python

```python
from stellar_sdk import Keypair

# Generate new keypair
keypair = Keypair.random()
print(f'Public Key: {keypair.public_key}')
print(f'Secret Key: {keypair.secret}')  # NEVER log in production

# Import from secret key
keypair = Keypair.from_secret('S...')

# Import from public key (read-only)
public_key = Keypair.from_public_key('GB...')
```

### Environment Variables

Store keys securely using environment variables:

```bash
# .env (never commit)
STELLAR_SECRET_KEY=S...
STELLAR_PUBLIC_KEY=GB...
CONTRACT_ID=C...
RPC_URL=https://soroban-testnet.stellar.org:443
```

#### TypeScript

```typescript
import dotenv from 'dotenv';
import { Keypair } from '@stellar/js-sdk';

dotenv.config();

const keypair = Keypair.fromSecret(process.env.STELLAR_SECRET_KEY!);
const contractId = process.env.CONTRACT_ID!;
```

#### Python

```python
import os
from dotenv import load_dotenv
from stellar_sdk import Keypair

load_dotenv()

keypair = Keypair.from_secret(os.getenv('STELLAR_SECRET_KEY'))
contract_id = os.getenv('CONTRACT_ID')
```

---

## Core Operations

### 1. Vouching

Stake tokens to vouch for a borrower.

#### TypeScript

```typescript
// Single vouch
const txHash = await client.vouch({
  voucher: keypair.publicKey(),
  borrower: 'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
  stake: '1000000000', // 100 XLM
  token: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
});

// Batch vouch (multiple borrowers)
const batchTxHash = await client.batchVouch({
  voucher: keypair.publicKey(),
  borrowers: [
    'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    'GCXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
  ],
  stakes: ['1000000000', '2000000000'], // 100 XLM, 200 XLM
  token: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
});
```

#### Python

```python
# Single vouch
tx_hash = await client.vouch(
    voucher=keypair.public_key,
    borrower='GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    stake='1000000000',
    token='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
)

# Batch vouch
batch_tx_hash = await client.batch_vouch(
    voucher=keypair.public_key,
    borrowers=[
        'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
        'GCXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    ],
    stakes=['1000000000', '2000000000'],
    token='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
)
```

### 2. Requesting a Loan

Borrow funds if sufficient vouches exist.

#### TypeScript

```typescript
const txHash = await client.requestLoan({
  borrower: keypair.publicKey(),
  amount: '500000000', // 50 XLM
  threshold: '1000000000', // Minimum 100 XLM in vouches
  loanPurpose: 'Business expansion',
  token: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
});
```

#### Python

```python
tx_hash = await client.request_loan(
    borrower=keypair.public_key,
    amount='500000000',
    threshold='1000000000',
    loan_purpose='Business expansion',
    token='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
)
```

### 3. Repaying a Loan

Repay loan principal and distribute yield to vouchers.

#### TypeScript

```typescript
// Repay with 2% yield
const principalAmount = '500000000'; // 50 XLM
const yieldAmount = '10000000'; // 1 XLM (2% of 50)
const totalRepayment = '510000000';

const txHash = await client.repay({
  borrower: keypair.publicKey(),
  payment: totalRepayment,
});
```

#### Python

```python
# Repay with 2% yield
principal_amount = '500000000'
yield_amount = '10000000'
total_repayment = '510000000'

tx_hash = await client.repay(
    borrower=keypair.public_key,
    payment=total_repayment,
)
```

### 4. Checking Eligibility

Verify if a borrower meets the stake threshold.

#### TypeScript

```typescript
const isEligible = await client.isEligible(
  'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
  '1000000000', // 100 XLM threshold
  'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4'
);

console.log('Eligible:', isEligible);
```

#### Python

```python
is_eligible = await client.is_eligible(
    'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    '1000000000',
    'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
)

print(f'Eligible: {is_eligible}')
```

### 5. Querying Loan Records

Retrieve loan and vouch information.

#### TypeScript

```typescript
// Get loan record
const loan = await client.getLoan('GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX');
if (loan) {
  console.log('Loan ID:', loan.id);
  console.log('Amount:', loan.amount);
  console.log('Status:', loan.status);
}

// Get all vouches
const vouches = await client.getVouches('GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX');
vouches.forEach((vouch) => {
  console.log(`Voucher: ${vouch.voucher}, Stake: ${vouch.stake}`);
});
```

#### Python

```python
# Get loan record
loan = await client.get_loan('GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX')
if loan:
    print(f'Loan ID: {loan.id}')
    print(f'Amount: {loan.amount}')
    print(f'Status: {loan.status}')

# Get all vouches
vouches = await client.get_vouches('GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX')
for vouch in vouches:
    print(f'Voucher: {vouch.voucher}, Stake: {vouch.stake}')
```

### 6. Getting Configuration

Retrieve protocol settings.

#### TypeScript

```typescript
const config = await client.getConfig();
console.log('Yield Rate:', config.yieldBps, 'bps');
console.log('Slash Rate:', config.slashBps, 'bps');
console.log('Min Loan:', config.minLoanAmount, 'stroops');
console.log('Max Loan:', config.maxLoanAmount, 'stroops');
```

#### Python

```python
config = await client.get_config()
print(f'Yield Rate: {config.yield_bps} bps')
print(f'Slash Rate: {config.slash_bps} bps')
print(f'Min Loan: {config.min_loan_amount} stroops')
print(f'Max Loan: {config.max_loan_amount} stroops')
```

---

## Error Handling

### Error Types

All operations return errors with specific codes:

| Code | Error | Meaning |
|------|-------|---------|
| 1 | InsufficientFunds | Not enough balance or stake |
| 2 | ActiveLoanExists | Borrower already has active loan |
| 3 | StakeOverflow | Stake amount too large |
| 4 | ZeroAddress | Invalid address provided |
| 5 | DuplicateVouch | Vouch already exists |
| 6 | NoActiveLoan | No loan found for borrower |
| 7 | ContractPaused | Contract is paused |
| 8 | LoanPastDeadline | Loan deadline has passed |

### TypeScript Error Handling

```typescript
try {
  const txHash = await client.vouch({
    voucher: keypair.publicKey(),
    borrower: 'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    stake: '1000000000',
    token: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
  });
  console.log('Success:', txHash);
} catch (error) {
  if (error.message.includes('InsufficientFunds')) {
    console.error('Not enough balance to vouch');
  } else if (error.message.includes('DuplicateVouch')) {
    console.error('Already vouching for this borrower');
  } else if (error.message.includes('ContractPaused')) {
    console.error('Contract is paused, try again later');
  } else {
    console.error('Unexpected error:', error.message);
  }
}
```

### Python Error Handling

```python
try:
    tx_hash = await client.vouch(
        voucher=keypair.public_key,
        borrower='GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
        stake='1000000000',
        token='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
    )
    print(f'Success: {tx_hash}')
except Exception as error:
    if 'InsufficientFunds' in str(error):
        print('Not enough balance to vouch')
    elif 'DuplicateVouch' in str(error):
        print('Already vouching for this borrower')
    elif 'ContractPaused' in str(error):
        print('Contract is paused, try again later')
    else:
        print(f'Unexpected error: {error}')
```

---

## Advanced Usage

### Stroops Conversion

All amounts are in stroops (1 XLM = 10,000,000 stroops).

#### TypeScript

```typescript
const XLM_TO_STROOPS = 10_000_000n;

function xlmToStroops(xlm: number): string {
  return (BigInt(Math.round(xlm * 10_000_000))).toString();
}

function stroopsToXlm(stroops: string): number {
  return Number(stroops) / 10_000_000;
}

// Usage
const stake = xlmToStroops(100); // "1000000000"
const xlm = stroopsToXlm(stake); // 100
```

#### Python

```python
XLM_TO_STROOPS = 10_000_000

def xlm_to_stroops(xlm: float) -> str:
    return str(int(xlm * XLM_TO_STROOPS))

def stroops_to_xlm(stroops: str) -> float:
    return int(stroops) / XLM_TO_STROOPS

# Usage
stake = xlm_to_stroops(100)  # "1000000000"
xlm = stroops_to_xlm(stake)  # 100.0
```

### Batch Operations

Process multiple operations efficiently.

#### TypeScript

```typescript
async function vouchForMultipleBorrowers(
  voucher: string,
  borrowers: string[],
  stakePerBorrower: string
) {
  const stakes = borrowers.map(() => stakePerBorrower);

  const txHash = await client.batchVouch({
    voucher,
    borrowers,
    stakes,
    token: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
  });

  return txHash;
}

// Usage
const borrowers = [
  'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
  'GCXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
  'GDXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
];

const txHash = await vouchForMultipleBorrowers(
  keypair.publicKey(),
  borrowers,
  '1000000000' // 100 XLM each
);
```

#### Python

```python
async def vouch_for_multiple_borrowers(
    voucher: str,
    borrowers: list[str],
    stake_per_borrower: str,
) -> str:
    stakes = [stake_per_borrower] * len(borrowers)

    tx_hash = await client.batch_vouch(
        voucher=voucher,
        borrowers=borrowers,
        stakes=stakes,
        token='CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4',
    )

    return tx_hash

# Usage
borrowers = [
    'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    'GCXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
    'GDXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX',
]

tx_hash = await vouch_for_multiple_borrowers(
    keypair.public_key,
    borrowers,
    '1000000000',  # 100 XLM each
)
```

### Monitoring Transactions

Track transaction status.

#### TypeScript

```typescript
import { SorobanRpc } from '@stellar/js-sdk';

async function waitForTransaction(
  txHash: string,
  maxAttempts: number = 30
): Promise<boolean> {
  const server = new SorobanRpc.Server('https://soroban-testnet.stellar.org:443');

  for (let i = 0; i < maxAttempts; i++) {
    try {
      const tx = await server.getTransaction(txHash);
      if (tx.status === 'SUCCESS') {
        console.log('Transaction confirmed');
        return true;
      } else if (tx.status === 'FAILED') {
        console.error('Transaction failed');
        return false;
      }
    } catch (error) {
      // Transaction not yet confirmed
    }

    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  console.error('Transaction confirmation timeout');
  return false;
}
```

#### Python

```python
import asyncio
from stellar_sdk import SorobanServer

async def wait_for_transaction(
    tx_hash: str,
    max_attempts: int = 30,
) -> bool:
    server = SorobanServer('https://soroban-testnet.stellar.org:443')

    for i in range(max_attempts):
        try:
            tx = await server.get_transaction(tx_hash)
            if tx.status == 'SUCCESS':
                print('Transaction confirmed')
                return True
            elif tx.status == 'FAILED':
                print('Transaction failed')
                return False
        except Exception:
            # Transaction not yet confirmed
            pass

        await asyncio.sleep(1)

    print('Transaction confirmation timeout')
    return False
```

---

## Examples

### Complete Loan Workflow

#### TypeScript

```typescript
import { QuorumCreditClient } from '@quorum-credit/sdk';
import { Keypair, Networks } from '@stellar/js-sdk';

async function completeLoanWorkflow() {
  const keypair = Keypair.fromSecret(process.env.STELLAR_SECRET_KEY!);
  const client = new QuorumCreditClient({
    contractId: process.env.CONTRACT_ID!,
    rpcUrl: 'https://soroban-testnet.stellar.org:443',
    networkPassphrase: Networks.TESTNET_NETWORK_PASSPHRASE,
    keypair,
  });

  const borrower = 'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX';
  const token = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4';

  // Step 1: Vouch for borrower
  console.log('Step 1: Vouching for borrower...');
  const vouchTx = await client.vouch({
    voucher: keypair.publicKey(),
    borrower,
    stake: '1000000000', // 100 XLM
    token,
  });
  console.log('Vouch transaction:', vouchTx);

  // Step 2: Check eligibility
  console.log('Step 2: Checking eligibility...');
  const isEligible = await client.isEligible(borrower, '1000000000', token);
  console.log('Eligible:', isEligible);

  if (!isEligible) {
    console.error('Borrower not eligible');
    return;
  }

  // Step 3: Request loan
  console.log('Step 3: Requesting loan...');
  const loanTx = await client.requestLoan({
    borrower,
    amount: '500000000', // 50 XLM
    threshold: '1000000000',
    loanPurpose: 'Business expansion',
    token,
  });
  console.log('Loan transaction:', loanTx);

  // Step 4: Get loan details
  console.log('Step 4: Getting loan details...');
  const loan = await client.getLoan(borrower);
  console.log('Loan:', loan);

  // Step 5: Repay loan
  console.log('Step 5: Repaying loan...');
  const repayTx = await client.repay({
    borrower,
    payment: '510000000', // 50 XLM + 2% yield
  });
  console.log('Repay transaction:', repayTx);

  console.log('✓ Loan workflow complete');
}

completeLoanWorkflow().catch(console.error);
```

#### Python

```python
import asyncio
import os
from dotenv import load_dotenv
from quorum_credit import QuorumCreditClient, ClientConfig
from stellar_sdk import Keypair, Networks

load_dotenv()

async def complete_loan_workflow():
    keypair = Keypair.from_secret(os.getenv('STELLAR_SECRET_KEY'))
    config = ClientConfig(
        contract_id=os.getenv('CONTRACT_ID'),
        rpc_url='https://soroban-testnet.stellar.org:443',
        network_passphrase=Networks.TESTNET_NETWORK_PASSPHRASE,
        keypair=keypair,
    )
    client = QuorumCreditClient(config)

    borrower = 'GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX'
    token = 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4'

    # Step 1: Vouch for borrower
    print('Step 1: Vouching for borrower...')
    vouch_tx = await client.vouch(
        voucher=keypair.public_key,
        borrower=borrower,
        stake='1000000000',
        token=token,
    )
    print(f'Vouch transaction: {vouch_tx}')

    # Step 2: Check eligibility
    print('Step 2: Checking eligibility...')
    is_eligible = await client.is_eligible(borrower, '1000000000', token)
    print(f'Eligible: {is_eligible}')

    if not is_eligible:
        print('Borrower not eligible')
        return

    # Step 3: Request loan
    print('Step 3: Requesting loan...')
    loan_tx = await client.request_loan(
        borrower=borrower,
        amount='500000000',
        threshold='1000000000',
        loan_purpose='Business expansion',
        token=token,
    )
    print(f'Loan transaction: {loan_tx}')

    # Step 4: Get loan details
    print('Step 4: Getting loan details...')
    loan = await client.get_loan(borrower)
    print(f'Loan: {loan}')

    # Step 5: Repay loan
    print('Step 5: Repaying loan...')
    repay_tx = await client.repay(
        borrower=borrower,
        payment='510000000',
    )
    print(f'Repay transaction: {repay_tx}')

    print('✓ Loan workflow complete')

asyncio.run(complete_loan_workflow())
```

---

## Troubleshooting

### Common Issues

#### "Contract not found"

```
Error: Contract not found at address CAAAA...
```

**Solution**: Verify the contract ID is correct and deployed on the network you're using.

```typescript
// Check contract exists
const server = new SorobanRpc.Server('https://soroban-testnet.stellar.org:443');
const contract = await server.getContractData(contractId);
```

#### "Insufficient funds"

```
Error: InsufficientFunds
```

**Solution**: Ensure you have enough balance and the contract has sufficient liquidity.

```typescript
// Check balance
const account = await server.getAccount(keypair.publicKey());
console.log('Balance:', account.balances);

// Check contract balance
const contractBalance = await server.getContractData(contractId);
```

#### "Transaction timeout"

```
Error: Transaction confirmation timeout
```

**Solution**: Increase timeout or check network status.

```typescript
// Increase timeout
const tx = new TransactionBuilder(account, {
  fee: BASE_FEE,
  networkPassphrase: Networks.TESTNET_NETWORK_PASSPHRASE,
})
  .setTimeout(60) // 60 seconds instead of 30
  .build();
```

#### "Invalid address"

```
Error: ZeroAddress
```

**Solution**: Verify addresses are valid Stellar addresses.

```typescript
// Validate address
import { StrKey } from '@stellar/js-sdk';

if (!StrKey.isValidEd25519PublicKey(address)) {
  throw new Error('Invalid Stellar address');
}
```

### Getting Help

- Check [QuorumCredit GitHub Issues](https://github.com/QuorumCredit/QuorumCredit/issues)
- Join [Stellar Developer Discord](https://discord.gg/stellardev)
- Review [Stellar Documentation](https://developers.stellar.org)
- Open an issue with detailed error logs

---

## API Reference

For complete API reference, see [OpenAPI Schema](../openapi.yaml).

### Conversion Utilities

**Stroops to XLM**: Divide by 10,000,000
**XLM to Stroops**: Multiply by 10,000,000

### Network Endpoints

- **Testnet RPC**: `https://soroban-testnet.stellar.org:443`
- **Mainnet RPC**: `https://rpc.mainnet.stellar.org:443`

### Token Addresses

- **Testnet XLM**: `CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4`
- **Mainnet XLM**: `CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4`
