# QuorumCredit SDKs

Official client libraries for integrating with QuorumCredit smart contract on Stellar Soroban.

## Available SDKs

### TypeScript/JavaScript

Full-featured SDK for Node.js and browser environments.

```bash
npm install @quorum-credit/sdk
```

**Features:**
- Full contract integration
- Type-safe API
- Async/await support
- Error handling
- Transaction monitoring

**Quick Start:**
```typescript
import { QuorumCreditClient } from '@quorum-credit/sdk';
import { Keypair, Networks } from '@stellar/js-sdk';

const client = new QuorumCreditClient({
  contractId: 'C...',
  rpcUrl: 'https://soroban-testnet.stellar.org:443',
  networkPassphrase: Networks.TESTNET_NETWORK_PASSPHRASE,
  keypair: Keypair.fromSecret('S...'),
});

// Vouch for a borrower
const txHash = await client.vouch({
  voucher: keypair.publicKey(),
  borrower: 'GB...',
  stake: '1000000000', // 100 XLM
  token: 'C...',
});
```

### Python

Async-first SDK for Python 3.8+.

```bash
pip install quorum-credit
```

**Features:**
- Async/await support
- Type hints
- Comprehensive error handling
- Dataclass-based models
- Full contract integration

**Quick Start:**
```python
import asyncio
from quorum_credit import QuorumCreditClient, ClientConfig
from stellar_sdk import Keypair, Networks

async def main():
    config = ClientConfig(
        contract_id='C...',
        rpc_url='https://soroban-testnet.stellar.org:443',
        network_passphrase=Networks.TESTNET_NETWORK_PASSPHRASE,
        keypair=Keypair.from_secret('S...'),
    )
    client = QuorumCreditClient(config)
    
    # Vouch for a borrower
    tx_hash = await client.vouch(
        voucher=keypair.public_key,
        borrower='GB...',
        stake='1000000000',
        token='C...',
    )

asyncio.run(main())
```

## Documentation

- [API Client Integration Guide](../docs/api-client-integration-guide.md) - Complete integration guide with examples
- [OpenAPI Schema](../openapi.yaml) - Full API specification
- [Production Deployment Guide](../docs/production-deployment-guide.md) - Deployment procedures

## Core Operations

All SDKs support the following operations:

### Vouching
- `vouch()` - Stake tokens for a single borrower
- `batch_vouch()` - Stake for multiple borrowers atomically
- `increase_stake()` - Add more stake to existing vouch
- `decrease_stake()` - Reduce stake amount
- `withdraw_vouch()` - Completely withdraw vouch

### Loans
- `request_loan()` - Request a loan if eligible
- `repay()` - Repay loan and distribute yield
- `get_loan()` - Get loan record
- `loan_status()` - Check loan status

### Queries
- `get_vouches()` - Get all vouches for borrower
- `is_eligible()` - Check loan eligibility
- `get_config()` - Get protocol configuration
- `get_admins()` - Get admin addresses
- `total_vouched()` - Get total vouched amount

### Admin Operations
- `slash()` - Slash defaulted borrower
- `pause()` - Pause contract
- `unpause()` - Resume contract
- `update_config()` - Update protocol settings

## Stroops Conversion

All amounts are in stroops (1 XLM = 10,000,000 stroops).

### TypeScript
```typescript
const xlmToStroops = (xlm: number) => (xlm * 10_000_000).toString();
const stroopsToXlm = (stroops: string) => Number(stroops) / 10_000_000;
```

### Python
```python
def xlm_to_stroops(xlm: float) -> str:
    return str(int(xlm * 10_000_000))

def stroops_to_xlm(stroops: str) -> float:
    return int(stroops) / 10_000_000
```

## Error Handling

All SDKs provide consistent error handling with specific error codes:

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

## Networks

### Testnet
- **RPC URL**: `https://soroban-testnet.stellar.org:443`
- **Network Passphrase**: `Test SDF Network ; September 2015`
- **XLM Token**: `CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4`

### Mainnet
- **RPC URL**: `https://rpc.mainnet.stellar.org:443`
- **Network Passphrase**: `Public Global Stellar Network ; September 2015`
- **XLM Token**: `CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4`

## Examples

### Complete Loan Workflow

**TypeScript:**
```typescript
// 1. Vouch for borrower
await client.vouch({
  voucher: keypair.publicKey(),
  borrower: 'GB...',
  stake: '1000000000',
  token: 'C...',
});

// 2. Check eligibility
const eligible = await client.isEligible('GB...', '1000000000', 'C...');

// 3. Request loan
await client.requestLoan({
  borrower: 'GB...',
  amount: '500000000',
  threshold: '1000000000',
  loanPurpose: 'Business expansion',
  token: 'C...',
});

// 4. Repay loan
await client.repay({
  borrower: 'GB...',
  payment: '510000000', // Principal + 2% yield
});
```

**Python:**
```python
# 1. Vouch for borrower
await client.vouch(
    voucher=keypair.public_key,
    borrower='GB...',
    stake='1000000000',
    token='C...',
)

# 2. Check eligibility
eligible = await client.is_eligible('GB...', '1000000000', 'C...')

# 3. Request loan
await client.request_loan(
    borrower='GB...',
    amount='500000000',
    threshold='1000000000',
    loan_purpose='Business expansion',
    token='C...',
)

# 4. Repay loan
await client.repay(
    borrower='GB...',
    payment='510000000',
)
```

## Development

### TypeScript SDK

```bash
cd sdk/typescript

# Install dependencies
npm install

# Build
npm run build

# Test
npm run test

# Lint
npm run lint

# Format
npm run format
```

### Python SDK

```bash
cd sdk/python

# Install dependencies
pip install -e ".[dev]"

# Test
pytest

# Lint
flake8 quorum_credit

# Type check
mypy quorum_credit

# Format
black quorum_credit
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details.

## License

MIT License - see [LICENSE](../LICENSE) for details.

## Support

- [GitHub Issues](https://github.com/QuorumCredit/QuorumCredit/issues)
- [Stellar Developer Discord](https://discord.gg/stellardev)
- [Documentation](../docs)

## Resources

- [Stellar Documentation](https://developers.stellar.org)
- [Soroban Docs](https://soroban.stellar.org)
- [QuorumCredit GitHub](https://github.com/QuorumCredit/QuorumCredit)
