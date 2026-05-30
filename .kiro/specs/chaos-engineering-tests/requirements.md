# Requirements Document

## Introduction

This feature adds a dedicated chaos engineering test suite to the QuorumCredit Soroban smart contract. Chaos engineering for a smart contract means deliberately exercising the contract under adversarial and edge-case conditions — zero balances, maximum vouchers, reentrancy simulation, invalid token addresses, expired loans, paused state, malformed inputs, and boundary arithmetic — to verify that the contract fails safely and predictably under every failure condition. The goal is to ensure that no combination of hostile inputs can cause an unexpected panic, leave state partially updated, or return an incorrect error variant. All tests live in a new module `src/chaos_test.rs` and are purely additive; no contract logic changes.

## Glossary

- **Contract**: The `QuorumCreditContract` Soroban smart contract.
- **Chaos Condition**: A deliberately adversarial or boundary input designed to trigger a failure path in the contract.
- **Safe Failure**: A contract invocation that returns a well-typed `ContractError` variant rather than panicking or leaving storage in an inconsistent state.
- **Unexpected Panic**: Any contract abort that is not a `ContractError` — e.g. an integer overflow, an unwrap on `None`, or an out-of-bounds index.
- **Partial State Update**: A scenario where some storage keys are written but a subsequent write fails, leaving the contract in an inconsistent intermediate state.
- **Boundary Value**: An input at the extreme edge of its valid range — zero, one, `i128::MAX`, `u32::MAX`, or the exact configured minimum/maximum.
- **Paused State**: The condition where `DataKey::Paused` is `true` and `Config.emergency_pause_enabled` is set, blocking all critical write paths.
- **Auto-slash**: The `auto_slash` function that marks a loan as defaulted when the current ledger timestamp exceeds the loan deadline.
- **Deadline**: The `LoanRecord.deadline` field — the ledger timestamp by which a loan must be repaid.
- **Chaos Test Helper**: A test utility function that sets up a specific adversarial state in the Soroban test environment without going through the normal contract entry points.

---

## Requirements

### Requirement 1: Zero and Boundary Value Input Handling

**User Story:** As a protocol auditor, I want the contract to handle zero and boundary value inputs without panicking so that no arithmetic edge case can crash the contract or corrupt state.

#### Acceptance Criteria

1. WHEN `vouch` is called with a stake amount of `0`, THE Contract SHALL return `ContractError::InvalidAmount` and SHALL NOT modify any storage.
2. WHEN `request_loan` is called with a loan amount of `0`, THE Contract SHALL return `ContractError::InvalidAmount` and SHALL NOT modify any storage.
3. WHEN `request_loan` is called with a loan amount of exactly `Config.min_loan_amount`, THE Contract SHALL succeed and disburse the loan.
4. WHEN `request_loan` is called with a loan amount of `Config.min_loan_amount - 1`, THE Contract SHALL return `ContractError::LoanBelowMinAmount`.
5. WHEN any arithmetic operation would overflow `i128::MAX` (e.g. stake accumulation), THE Contract SHALL return `ContractError::StakeOverflow` rather than panicking.
6. WHEN `vouch` is called with a stake of `i128::MAX`, THE Contract SHALL either succeed or return `ContractError::StakeOverflow`; it SHALL NOT panic.
7. WHEN `request_loan` is called with an amount of `i128::MAX`, THE Contract SHALL return `ContractError::LoanExceedsMaxAmount` or `ContractError::InvalidAmount`; it SHALL NOT panic.

---

### Requirement 2: State Corruption Scenario Prevention

**User Story:** As a protocol auditor, I want the contract to reject duplicate or out-of-order state transitions so that no sequence of calls can leave the contract in an inconsistent state.

#### Acceptance Criteria

1. WHEN `vouch` is called twice by the same voucher for the same borrower and token, THE Contract SHALL return `ContractError::DuplicateVouch` on the second call and SHALL NOT modify the existing vouch record.
2. WHEN `repay` is called on a loan that has already been fully repaid (status `Repaid`), THE Contract SHALL return `ContractError::NoActiveLoan`.
3. WHEN `repay` is called on a loan that has been slashed (status `Defaulted`), THE Contract SHALL return `ContractError::NoActiveLoan`.
4. WHEN a slash is attempted on a borrower whose loan has already been repaid, THE Contract SHALL return an appropriate error (`ContractError::NoActiveLoan` or `ContractError::SlashAlreadyExecuted`) and SHALL NOT modify voucher stakes.
5. WHEN `repay` is called after a slash has been executed for the same loan, THE Contract SHALL return `ContractError::NoActiveLoan` and SHALL NOT alter the `DefaultCount` or voucher stakes a second time.
6. WHEN `vote_slash` is called for a borrower who has no active loan, THE Contract SHALL return `ContractError::NoActiveLoan`.

---

### Requirement 3: Paused State Chaos

**User Story:** As a protocol operator, I want all critical write functions to be blocked when the contract is paused so that no state changes can occur during an emergency pause.

#### Acceptance Criteria

1. WHEN the contract is paused and `vouch` is called, THE Contract SHALL return `ContractError::ContractPaused`.
2. WHEN the contract is paused and `request_loan` is called, THE Contract SHALL return `ContractError::ContractPaused`.
3. WHEN the contract is paused and `repay` is called, THE Contract SHALL return `ContractError::ContractPaused`.
4. WHEN the contract is paused and `auto_slash` is called, THE Contract SHALL return `ContractError::ContractPaused`.
5. WHEN the contract is paused and `vote_slash` is called, THE Contract SHALL return `ContractError::ContractPaused`.
6. WHEN the contract is unpaused after a pause, ALL previously blocked functions SHALL accept valid inputs and execute normally.
7. WHEN the contract is paused, read-only functions (`get_loan`, `get_vouches`, `get_credit_score`) SHALL continue to return data without error.

---

### Requirement 4: Token Failure Scenarios

**User Story:** As a protocol auditor, I want the contract to handle token-related failures gracefully so that a bad token address or insufficient balance cannot corrupt contract state.

#### Acceptance Criteria

1. WHEN `vouch` is called with a token address that is not in `Config.allowed_tokens` and is not `Config.token`, THE Contract SHALL return `ContractError::InvalidToken`.
2. WHEN `request_loan` is called but the contract's token balance is insufficient to cover the loan amount, THE Contract SHALL return `ContractError::InsufficientFunds` and SHALL NOT create a loan record.
3. WHEN a voucher's token balance is zero at the time of `vouch`, THE Contract SHALL return `ContractError::InsufficientFunds` and SHALL NOT create a vouch record.
4. WHEN `repay` is called but the borrower's token balance is insufficient to cover the repayment amount, THE Contract SHALL return `ContractError::InsufficientFunds` and SHALL NOT alter the loan status.
5. WHEN `vouch` is called with the zero address (`Address::default()` or equivalent), THE Contract SHALL return `ContractError::ZeroAddress` or `ContractError::InvalidToken`.

---

### Requirement 5: Deadline and Timing Chaos

**User Story:** As a protocol auditor, I want deadline-sensitive functions to behave correctly at exact boundary timestamps so that no timing edge case can bypass deadline enforcement.

#### Acceptance Criteria

1. WHEN `auto_slash` is called at a ledger timestamp exactly equal to `LoanRecord.deadline`, THE Contract SHALL treat the loan as expired and execute the slash.
2. WHEN `repay` is called at a ledger timestamp exactly equal to `LoanRecord.deadline`, THE Contract SHALL accept the repayment as on-time and mark the loan `Repaid`.
3. WHEN `repay` is called at a ledger timestamp of `LoanRecord.deadline + 1`, THE Contract SHALL return `ContractError::LoanPastDeadline`.
4. WHEN `auto_slash` is called on a loan that has already been slashed, THE Contract SHALL return `ContractError::SlashAlreadyExecuted` or `ContractError::NoActiveLoan` and SHALL NOT slash vouchers a second time.
5. WHEN `auto_slash` is called on a loan that has been repaid, THE Contract SHALL return `ContractError::NoActiveLoan` and SHALL NOT modify any state.
6. WHEN `vote_slash` is called after the loan deadline has passed but before `auto_slash` has been called, THE Contract SHALL still accept the vote and process the slash normally.

---

### Requirement 6: Multi-Voucher Stress Scenarios

**User Story:** As a protocol auditor, I want the contract to handle extreme voucher counts correctly so that boundary conditions on the voucher list do not cause panics or incorrect behaviour.

#### Acceptance Criteria

1. WHEN `request_loan` is called with exactly `Config.max_vouchers_per_borrower` active vouchers, THE Contract SHALL succeed and disburse the loan.
2. WHEN `vouch` is called and the borrower already has `Config.max_vouchers_per_borrower` vouchers, THE Contract SHALL return `ContractError::MaxVouchersPerBorrowerExceeded`.
3. WHEN `request_loan` is called with zero vouchers, THE Contract SHALL return `ContractError::InsufficientVouchers`.
4. WHEN `request_loan` is called with exactly one voucher whose stake meets the minimum threshold, THE Contract SHALL succeed.
5. WHEN a slash is executed against a borrower with the maximum number of vouchers, THE Contract SHALL slash all vouchers proportionally and SHALL NOT panic or skip any voucher.
6. WHEN all vouchers of a borrower have zero stake (e.g. after a prior slash), THE Contract SHALL return `ContractError::InsufficientFunds` or `ContractError::InsufficientVouchers` on a new `request_loan` call.

---

### Requirement 7: Governance Chaos

**User Story:** As a protocol auditor, I want governance functions to reject invalid voting states so that no sequence of governance calls can bypass quorum requirements or execute proposals prematurely.

#### Acceptance Criteria

1. WHEN `vote_slash` is called by an address with zero stake, THE Contract SHALL return `ContractError::InsufficientFunds` or `ContractError::NotGovernanceParticipant`.
2. WHEN `vote_slash` is called by the same address twice for the same borrower, THE Contract SHALL return `ContractError::AlreadyVoted` on the second call.
3. WHEN `finalize_slash_threshold_proposal` is called before the voting period has ended, THE Contract SHALL return an appropriate error (`ContractError::VotingPeriodEnded` or equivalent) and SHALL NOT finalize the proposal.
4. WHEN `vote_slash` is called after quorum has already been reached and the slash auto-executed, THE Contract SHALL return `ContractError::SlashAlreadyExecuted`.
5. WHEN a governance proposal is finalized and then `vote_slash` is called again for the same proposal, THE Contract SHALL return `ContractError::SlashAlreadyExecuted` or `ContractError::ProposalAlreadyFinalized`.
6. WHEN `propose_slash_threshold` is called by an address that is not an admin and has no stake, THE Contract SHALL return `ContractError::UnauthorizedCaller` or `ContractError::NotGovernanceParticipant`.

---

### Requirement 8: Test Coverage and Infrastructure

**User Story:** As a developer, I want a well-structured chaos test module with reusable helpers so that new chaos scenarios can be added quickly and consistently.

#### Acceptance Criteria

1. THE chaos test suite SHALL be located in `src/chaos_test.rs` and registered under `#[cfg(test)]` in `src/lib.rs`.
2. THE module SHALL provide helper functions for setting up each category of adversarial state (paused contract, zero-balance voucher, max-voucher borrower, expired loan, etc.) without duplicating setup code across tests.
3. EACH test function SHALL include a comment identifying the chaos category and the requirement it validates.
4. THE test suite SHALL run to completion with `cargo test` without any unexpected panics or test framework errors.
5. THE test suite SHALL include at least one test per acceptance criterion in Requirements 1 through 7.
6. ALL chaos tests SHALL use the Soroban test environment (`soroban_sdk::testutils`) and SHALL NOT require any external network or token contract deployment.
