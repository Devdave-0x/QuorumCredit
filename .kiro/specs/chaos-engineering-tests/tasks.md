# Implementation Plan: Chaos Engineering Tests

## Overview

Purely additive: create `src/chaos_test.rs` with setup helpers and ~35 test functions covering all seven chaos categories. No contract logic changes. All tests use `try_*` client methods and assert specific `ContractError` variants.

## Tasks

- [ ] 1. Create `src/chaos_test.rs` with module skeleton and setup helpers
  - [ ] 1.1 Define `ChaosFixture` struct holding `contract_id`, `token_addr`, `admin`, `borrower`, `voucher`
    - _Requirements: 8.2_
  - [ ] 1.2 Implement `setup_standard(env: &Env) -> ChaosFixture`
    - Initialise contract, register token, mint tokens to voucher and contract
    - _Requirements: 8.1, 8.6_
  - [ ] 1.3 Implement `setup_paused(env: &Env) -> ChaosFixture`
    - Call `setup_standard`, then call `client.pause(admin_signers)`
    - _Requirements: 3.1_
  - [ ] 1.4 Implement `setup_max_vouchers(env: &Env) -> ChaosFixture`
    - Call `setup_standard`, then vouch with `DEFAULT_MAX_VOUCHERS_PER_BORROWER` distinct vouchers
    - _Requirements: 6.1_
  - [ ] 1.5 Implement `setup_expired_loan(env: &Env) -> ChaosFixture`
    - Call `setup_standard`, vouch, request_loan, then advance ledger timestamp past `loan.deadline`
    - _Requirements: 5.1_
  - [ ] 1.6 Implement `setup_zero_balance(env: &Env) -> ChaosFixture`
    - Call `setup_standard` but do NOT mint tokens to the voucher
    - _Requirements: 4.3_
  - [ ] 1.7 Register the module in `src/lib.rs`: `#[cfg(test)] mod chaos_test;`
    - _Requirements: 8.1_
  - [ ] 1.8 Run `cargo check` to confirm the skeleton compiles

- [ ] 2. Implement boundary value tests (Requirement 1)
  - [ ] 2.1 `test_chaos_boundary_zero_stake` — `vouch` with stake=0 → `ContractError::InvalidAmount`
    - _Requirements: 1.1_
  - [ ] 2.2 `test_chaos_boundary_zero_loan_amount` — `request_loan` with amount=0 → `ContractError::InvalidAmount`
    - _Requirements: 1.2_
  - [ ] 2.3 `test_chaos_boundary_min_loan_amount_exact` — `request_loan` with amount=`min_loan_amount` → succeeds
    - _Requirements: 1.3_
  - [ ] 2.4 `test_chaos_boundary_below_min_loan_amount` — `request_loan` with amount=`min_loan_amount - 1` → `ContractError::LoanBelowMinAmount`
    - _Requirements: 1.4_
  - [ ] 2.5 `test_chaos_boundary_max_i128_stake` — `vouch` with stake=`i128::MAX` → succeeds or `ContractError::StakeOverflow`; must NOT panic
    - _Requirements: 1.6_

- [ ] 3. Implement state corruption tests (Requirement 2)
  - [ ] 3.1 `test_chaos_corruption_duplicate_vouch` — second `vouch` from same voucher → `ContractError::DuplicateVouch`; vouch record unchanged
    - _Requirements: 2.1_
  - [ ] 3.2 `test_chaos_corruption_repay_already_repaid` — `repay` on a repaid loan → `ContractError::NoActiveLoan`
    - _Requirements: 2.2_
  - [ ] 3.3 `test_chaos_corruption_repay_after_slash` — `repay` after slash executed → `ContractError::NoActiveLoan`
    - _Requirements: 2.3_
  - [ ] 3.4 `test_chaos_corruption_slash_after_repay` — slash attempt on repaid loan → appropriate error; voucher stakes unchanged
    - _Requirements: 2.4_
  - [ ] 3.5 `test_chaos_corruption_vote_slash_no_loan` — `vote_slash` with no active loan → `ContractError::NoActiveLoan`
    - _Requirements: 2.6_

- [ ] 4. Implement paused state tests (Requirement 3)
  - [ ] 4.1 `test_chaos_paused_vouch_blocked` → `ContractError::ContractPaused`
    - _Requirements: 3.1_
  - [ ] 4.2 `test_chaos_paused_request_loan_blocked` → `ContractError::ContractPaused`
    - _Requirements: 3.2_
  - [ ] 4.3 `test_chaos_paused_repay_blocked` → `ContractError::ContractPaused`
    - _Requirements: 3.3_
  - [ ] 4.4 `test_chaos_paused_auto_slash_blocked` → `ContractError::ContractPaused`
    - _Requirements: 3.4_
  - [ ] 4.5 `test_chaos_paused_vote_slash_blocked` → `ContractError::ContractPaused`
    - _Requirements: 3.5_
  - [ ] 4.6 `test_chaos_paused_reads_still_work` — `get_loan`, `get_vouches` succeed while paused
    - _Requirements: 3.7_
  - [ ] 4.7 `test_chaos_paused_unpause_restores_vouch` — unpause then vouch succeeds
    - _Requirements: 3.6_

- [ ] 5. Checkpoint — verify all tests so far compile and pass
  - Run `cargo check`
  - Run `cargo test chaos` to run only the chaos module
  - Ask the user if questions arise.

- [ ] 6. Implement token failure tests (Requirement 4)
  - [ ] 6.1 `test_chaos_token_invalid_token_address` — `vouch` with unlisted token → `ContractError::InvalidToken`
    - _Requirements: 4.1_
  - [ ] 6.2 `test_chaos_token_insufficient_contract_balance` — `request_loan` when contract balance < amount → `ContractError::InsufficientFunds`; no loan record created
    - _Requirements: 4.2_
  - [ ] 6.3 `test_chaos_token_zero_balance_voucher` — `vouch` when voucher balance=0 → `ContractError::InsufficientFunds`; no vouch record created
    - _Requirements: 4.3_
  - [ ] 6.4 `test_chaos_token_insufficient_borrower_balance_repay` — `repay` when borrower balance < owed → `ContractError::InsufficientFunds`; loan status unchanged
    - _Requirements: 4.4_

- [ ] 7. Implement deadline and timing chaos tests (Requirement 5)
  - [ ] 7.1 `test_chaos_deadline_auto_slash_at_exact_deadline` — timestamp == deadline → slash executes
    - _Requirements: 5.1_
  - [ ] 7.2 `test_chaos_deadline_repay_at_exact_deadline` — timestamp == deadline → repay succeeds
    - _Requirements: 5.2_
  - [ ] 7.3 `test_chaos_deadline_repay_one_second_past` — timestamp == deadline+1 → `ContractError::LoanPastDeadline`
    - _Requirements: 5.3_
  - [ ] 7.4 `test_chaos_deadline_auto_slash_already_slashed` — auto_slash on defaulted loan → error; no double-slash
    - _Requirements: 5.4_
  - [ ] 7.5 `test_chaos_deadline_auto_slash_on_repaid_loan` — auto_slash on repaid loan → `ContractError::NoActiveLoan`
    - _Requirements: 5.5_

- [ ] 8. Implement multi-voucher stress tests (Requirement 6)
  - [ ] 8.1 `test_chaos_vouchers_max_count_loan_succeeds` — exactly max vouchers → `request_loan` succeeds
    - _Requirements: 6.1_
  - [ ] 8.2 `test_chaos_vouchers_exceed_max` — max+1 vouchers → `ContractError::MaxVouchersPerBorrowerExceeded`
    - _Requirements: 6.2_
  - [ ] 8.3 `test_chaos_vouchers_zero_vouchers_loan_fails` — no vouches → `ContractError::InsufficientVouchers` or `ContractError::InsufficientFunds`
    - _Requirements: 6.3_
  - [ ] 8.4 `test_chaos_vouchers_single_voucher_meets_threshold` — one voucher at threshold → loan succeeds
    - _Requirements: 6.4_
  - [ ] 8.5 `test_chaos_vouchers_slash_max_vouchers` — slash with max vouchers → all slashed proportionally; no panic
    - Use `env.budget().reset_unlimited()` to avoid budget exhaustion
    - _Requirements: 6.5_

- [ ] 9. Implement governance chaos tests (Requirement 7)
  - [ ] 9.1 `test_chaos_governance_vote_zero_stake` — `vote_slash` with zero stake → appropriate error
    - _Requirements: 7.1_
  - [ ] 9.2 `test_chaos_governance_duplicate_vote` — same address votes twice → `ContractError::AlreadyVoted`
    - _Requirements: 7.2_
  - [ ] 9.3 `test_chaos_governance_finalize_before_period_ends` — finalize before voting period → appropriate error
    - _Requirements: 7.3_
  - [ ] 9.4 `test_chaos_governance_vote_after_quorum` — vote after quorum reached and slash executed → `ContractError::SlashAlreadyExecuted`
    - _Requirements: 7.4_

- [ ] 10. Final checkpoint
  - Run `cargo check` — must pass without errors
  - Run `cargo clippy` — address all warnings
  - Run `cargo test` — all tests must pass including all chaos tests
  - Confirm at least 35 test functions exist in `src/chaos_test.rs`
  - Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- Use `env.budget().reset_unlimited()` in stress tests with max vouchers to avoid hitting the default CPU budget
- All `try_*` calls return `Result<Result<T, ContractError>, InvokeError>` — assert with `Err(Ok(ContractError::XYZ))`
- `should_panic` tests are acceptable for cases where the contract panics (e.g. `auto_slash` before deadline) rather than returning a `ContractError`
