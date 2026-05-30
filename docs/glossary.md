# QuorumCredit Glossary

A comprehensive glossary of terms and concepts used in the QuorumCredit protocol.

## Core Concepts

### Proof of Trust (PoT)
A consensus mechanism that replaces asset collateral with social collateral. Borrowers are backed by their trust network (vouchers) rather than locked-up assets. Inspired by Stellar's Federated Byzantine Agreement (FBA).

### Social Collateral
Staked XLM from vouchers that backs a borrower's loan. Unlike traditional collateral, social collateral is not over-collateralized and is slashed (not liquidated) on default.

### Quorum Slice
A borrower's personal set of trusted vouchers. Inspired by Stellar's FBA, where each node selects its own quorum slice of trusted peers. In QuorumCredit, a borrower's quorum slice determines their loan eligibility.

### Trust Graph
The network of relationships between borrowers and vouchers. Each edge represents a vouch relationship. The trust graph is decentralized and user-defined.

## Participants

### Borrower
An individual or entity seeking a loan. Borrowers become eligible for loans once their total vouched stake meets the minimum threshold. Borrowers repay loans with interest (yield).

### Voucher
An individual or entity that stakes XLM to back a borrower. Vouchers earn 2% yield on their stake if the loan is repaid. Vouchers lose 50% of their stake if the borrower defaults.

### Admin
An address with governance privileges. Admins can:
- Pause/unpause the contract
- Update configuration (yield rate, slash rate, etc.)
- Slash defaulted borrowers
- Manage allowed tokens
- Withdraw protocol fees

### Deployer
The address that deployed the contract. The deployer must sign the `initialize` transaction to prevent front-running attacks.

## Financial Terms

### Stroop
Stellar's smallest indivisible unit. 1 XLM = 10,000,000 stroops. All contract amounts are denominated in stroops.

### Yield
Interest paid to vouchers on successful loan repayment. Default rate: 2% (200 basis points). Calculated as `stake * yield_bps / 10_000`.

### Slash
Penalty applied to vouchers on borrower default. Default rate: 50% (5000 basis points). Calculated as `stake * slash_bps / 10_000`.

### Basis Points (BPS)
Unit of measurement for percentages. 1 BPS = 0.01% = 1/10,000. Used for yield and slash rates.
- 100 BPS = 1%
- 200 BPS = 2%
- 5000 BPS = 50%

### Yield Reserve
Pre-funded pool of XLM held by the contract to pay yield to vouchers. Must be maintained at sufficient levels to cover principal + yield for all active loans.

### Protocol Fee
Optional fee collected by the protocol on loan disbursement or repayment. Accumulated in the fee treasury.

### Fee Treasury
Contract-held balance of accumulated protocol fees. Admins can withdraw fees for protocol maintenance.

### Slash Treasury
Contract-held balance of slashed funds from defaulted loans. Admins can withdraw slashed funds for protocol use or redistribution.

## Loan Terms

### Loan Record
Data structure containing:
- Borrower address
- Loan amount (principal)
- Amount repaid
- Total yield locked in
- Loan status (Active, Repaid, Defaulted)
- Created timestamp
- Disbursement timestamp
- Repayment timestamp (if repaid)
- Deadline
- Loan purpose
- Token address

### Loan Status
State of a loan:
- **None:** No active loan
- **Active:** Loan disbursed, awaiting repayment
- **Repaid:** Loan fully repaid with yield distributed
- **Defaulted:** Loan past deadline without full repayment, slashed

### Loan Eligibility
A borrower is eligible for a loan if:
1. Total vouched stake ≥ requested threshold
2. Requested amount ≤ max loan amount
3. Borrower not blacklisted
4. Borrower has no active loan
5. All vouches are older than MIN_VOUCH_AGE

### Loan Deadline
Timestamp by which a loan must be repaid. Default: 30 days from disbursement. After deadline, loan can only be resolved via slash.

### Grace Period
Time window after loan deadline during which borrower can still repay without slash. Default: 7 days. After grace period, only slash is allowed.

### Loan Purpose
Free-form string describing the intended use of loan funds. Used for transparency and record-keeping.

### Co-Borrower
Additional borrower on a loan (optional). Co-borrowers share repayment responsibility.

## Vouch Terms

### Vouch Record
Data structure containing:
- Voucher address
- Staked amount
- Vouch timestamp
- Token address

### Vouch
The act of staking XLM to back a borrower. A vouch creates a trust relationship and increases the borrower's loan eligibility.

### Minimum Stake
Minimum amount a voucher must stake. Default: 50 stroops. Enforced to ensure non-zero yield calculation.

### Vouch Age
Time elapsed since a vouch was created. Used to prevent flash-loan attacks. Minimum vouch age before loan eligibility: 24 hours (configurable).

### Vouch Cooldown
Minimum time between successive vouches by the same voucher for the same borrower. Default: 24 hours. Prevents rapid stake accumulation.

### Batch Vouch
Atomic operation to vouch for multiple borrowers in a single transaction. All-or-nothing semantics: if any vouch fails validation, entire batch is rejected.

### Increase Stake
Operation to add more stake to an existing vouch. Increases borrower's loan eligibility.

### Decrease Stake
Operation to reduce stake in an existing vouch. Cannot reduce below minimum stake. Cannot reduce if borrower has active loan.

### Withdraw Vouch
Operation to completely remove a vouch and return staked funds. Cannot withdraw if borrower has active loan.

## Governance Terms

### Admin Threshold
Minimum number of admin signatures required for admin operations. Example: 2-of-3 multisig requires 2 signatures from 3 admins.

### Multisig
Multi-signature scheme requiring multiple signers to authorize operations. Prevents single-admin abuse.

### Slash Vote
Governance mechanism for vouchers to vote on slashing a defaulted borrower. Requires quorum (default: 50% of total stake) to execute.

### Slash Quorum
Minimum percentage of voucher stake required to approve a slash. Default: 50%. Prevents minority vouchers from slashing.

### Timelock
Delay mechanism for governance operations. Allows community to review and potentially veto changes before execution.

### Governance Token
Optional token used for voting rights (future feature). Separate from staking token.

## Technical Terms

### Contract
Smart contract deployed on Stellar Soroban. Implements the QuorumCredit protocol logic.

### WASM
WebAssembly binary format. QuorumCredit contract is compiled to WASM for deployment on Soroban.

### Soroban
Stellar's smart contract platform. Enables complex contract logic on Stellar.

### Stellar
Blockchain platform optimized for payments and asset transfers. QuorumCredit is built on Stellar.

### SEP-41
Stellar Enhancement Proposal for token interface. Defines standard token contract interface.

### RPC
Remote Procedure Call. Interface for interacting with Soroban contracts.

### Event
Emitted by contract to signal state changes. Events are indexed and queryable off-chain.

### Storage
Persistent data storage on Soroban. Stores contract state (loans, vouches, config).

### Data Key
Identifier for storage entries. Examples: `DataKey::Loan(id)`, `DataKey::Config`.

### Pause
Contract state where all state-mutating operations are blocked. Used for emergency stops and upgrades.

### Upgrade
Process of replacing contract WASM with new version. Requires admin multisig approval.

## Error Terms

### InsufficientFunds
Error when contract or user lacks sufficient balance for operation.

### ActiveLoanExists
Error when attempting to vouch for borrower with active loan.

### DuplicateVouch
Error when attempting to vouch for same borrower twice without using increase_stake.

### NoActiveLoan
Error when attempting to repay or slash borrower with no active loan.

### ContractPaused
Error when attempting state-mutating operation on paused contract.

### UnauthorizedCaller
Error when caller lacks required permissions for operation.

### InvalidAmount
Error when amount parameter fails validation (e.g., negative, zero, or exceeds limits).

### MinStakeNotMet
Error when vouch stake is below minimum required amount.

### LoanExceedsMaxAmount
Error when requested loan amount exceeds protocol maximum.

### InsufficientVouchers
Error when borrower has fewer vouchers than required minimum.

### Blacklisted
Error when borrower is on protocol blacklist.

### VouchTooRecent
Error when vouch is too new (before MIN_VOUCH_AGE) for loan eligibility.

## Operational Terms

### Deployment
Process of uploading contract to Stellar network and initializing it.

### Testnet
Stellar test network for development and testing. Separate from mainnet.

### Mainnet
Stellar production network. Real XLM and assets.

### Monitoring
Continuous observation of contract health via metrics and alerts.

### Alerting
Automated notifications when metrics exceed thresholds.

### Runbook
Documented procedures for responding to alerts and incidents.

### Synthetic Monitoring
Periodic test transactions to verify contract health.

### Metrics
Quantitative measurements of contract behavior (loan volume, error rate, etc.).

### Dashboard
Visual representation of metrics for monitoring.

### Audit
Security review of contract code and operations.

### Vulnerability
Security weakness that could be exploited.

### Patch
Code fix for vulnerability or bug.

### Rollback
Reverting to previous contract version after failed upgrade.

## Economic Terms

### Microlending
Lending of small amounts to underserved populations.

### Underbanked
Individuals without access to traditional banking services.

### Credit Score
Numerical rating of borrower creditworthiness. QuorumCredit uses on-chain repayment history.

### Default
Failure to repay loan by deadline.

### Repayment
Returning borrowed funds plus yield to contract.

### Interest
Fee paid by borrower for use of funds. In QuorumCredit, paid as yield to vouchers.

### Collateral
Assets pledged to secure a loan. In QuorumCredit, social collateral (vouches) replaces asset collateral.

### Over-Collateralization
Requiring collateral value > loan value. Traditional DeFi requires 150%+ over-collateralization. QuorumCredit requires 0% (social collateral).

### Liquidity
Availability of funds for lending. Determined by yield reserve balance.

### Solvency
Ability to meet financial obligations. QuorumCredit maintains solvency via hard-cap logic.

## Abbreviations

| Abbreviation | Full Term |
|---|---|
| BPS | Basis Points |
| PoT | Proof of Trust |
| FBA | Federated Byzantine Agreement |
| XLM | Stellar Lumens (native asset) |
| WASM | WebAssembly |
| RPC | Remote Procedure Call |
| SEP | Stellar Enhancement Proposal |
| ADR | Architecture Decision Record |
| CI/CD | Continuous Integration / Continuous Deployment |
| KYC | Know Your Customer |
| AML | Anti-Money Laundering |
| DAO | Decentralized Autonomous Organization |
| DeFi | Decentralized Finance |
| TVL | Total Value Locked |
| APY | Annual Percentage Yield |
| APR | Annual Percentage Rate |

## Related Concepts

### Federated Byzantine Agreement (FBA)
Consensus mechanism used by Stellar. Each node selects its own quorum slice of trusted peers. QuorumCredit mirrors this with borrower quorum slices.

### Decentralized Finance (DeFi)
Financial services built on blockchain without intermediaries. QuorumCredit is a DeFi protocol.

### Smart Contract
Self-executing code on blockchain. QuorumCredit is a smart contract.

### Blockchain
Distributed ledger technology. Stellar is a blockchain.

### Cryptocurrency
Digital currency based on cryptography. XLM is a cryptocurrency.

### Token
Digital asset on blockchain. XLM is Stellar's native token.

### Wallet
Software for managing cryptocurrency keys and balances.

### Key Pair
Cryptographic pair of public and private keys. Used for signing transactions.

### Signature
Cryptographic proof of authorization. Required for all contract operations.

### Transaction
Atomic operation on blockchain. Includes contract invocations.

### Ledger
Distributed record of all transactions. Maintained by Stellar network.

## See Also

- [README.md](../README.md) - Protocol overview
- [API Reference](../README.md#api-reference) - Contract functions
- [Error Reference](../README.md#error-reference) - Error codes
- [Architecture Decision Records](./adr/) - Design decisions
