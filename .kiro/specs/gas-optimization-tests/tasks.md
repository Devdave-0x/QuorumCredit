# Implementation Plan: Gas Optimization Tests

## Overview

Create `src/gas_test.rs` with measurement and regression tests for all core functions, document budgets in `docs/gas-budgets.md`, and implement at least one concrete optimization. No contract API changes.

## Tasks

- [ ] 1. Create `src/gas_test.rs` skeleton with setup helper and budget constants
  - [ ] 1.1 Define `GasFixture` struct holding `contract_id`, `token_addr`, `admin`, `borrower`, `voucher`
    - _Requirements: 6.1_
  - [ ] 1.2 Implement `setup(env: &Env) -> GasFixture`
    - Initialise contract, register token, mint tokens to voucher and contract
    - Consistent with the pattern in existing test modules
    - _Requirements: 6.1_
  - [ ] 1.3 Define placeholder budget constants at the top of the module (set to `u64::MAX` initially)
    - One `CPU_BUDGET_<FUNCTION>_<SCENARIO>` and `MEM_BUDGET_<FUNCTION>_<SCENARIO>` pair per function per scenario
    - _Requirements: 3.3, 6.2_
  - [ ] 1.4 Add a module-level comment explaining how to run measurement tests and how to update budgets
    - _Requirements: 6.4_
  - [ ] 1.5 Register the module in `src/lib.rs`: `#[cfg(test)] mod gas_test;`
    - _Requirements: 1.5_
  - [ ] 1.6 Run `cargo check` to confirm the skeleton compiles

- [ ] 2. Write measurement tests for all core functions
  - [ ] 2.1 `measure_vouch_typical` — 1 existing vouch, measure `vouch` call
    - Call `env.budget().reset_default()` before, read counts after, `println!`
    - _Requirements: 1.1, 1.2, 1.4_
  - [ ] 2.2 `measure_vouch_worst` — 49 existing vouches, measure `vouch` call
    - Use `env.budget().reset_unlimited()` during setup, then `reset_default()` before the measured call
    - _Requirements: 1.3_
  - [ ] 2.3 `measure_request_loan_typical` — 1 voucher
    - _Requirements: 1.1_
  - [ ] 2.4 `measure_request_loan_worst` — 50 vouchers
    - _Requirements: 1.3_
  - [ ] 2.5 `measure_repay_typical` — 1 voucher
    - _Requirements: 1.1_
  - [ ] 2.6 `measure_repay_worst` — 50 vouchers
    - _Requirements: 1.3_
  - [ ] 2.7 `measure_slash_typical` — 1 voucher, via `vote_slash` + `execute_pending_slash`
    - _Requirements: 1.1_
  - [ ] 2.8 `measure_slash_worst` — 50 vouchers
    - _Requirements: 1.3_
  - [ ] 2.9 `measure_auto_slash_typical` — 1 voucher, advance timestamp past deadline
    - _Requirements: 1.1_
  - [ ] 2.10 `measure_auto_slash_worst` — 50 vouchers
    - _Requirements: 1.3_
  - [ ] 2.11 `measure_withdraw_vouch_typical` — 1 voucher
    - _Requirements: 1.1_
  - [ ] 2.12 `measure_batch_vouch_typical` — 1 borrower
    - _Requirements: 1.1_
  - [ ] 2.13 `measure_batch_vouch_worst` — 50 borrowers
    - _Requirements: 1.3_

- [ ] 3. Run measurement tests and record baseline values
  - Run `cargo test gas::measure -- --nocapture` to print all CPU and memory counts
  - Record each value in a scratch document
  - _Requirements: 1.4_

- [ ] 4. Create `docs/gas-budgets.md` with baseline measurements
  - [ ] 4.1 Create the file with the budget table (columns: Function, Scenario, CPU Budget, Memory Budget, Notes)
    - Set each budget = measured baseline × 1.5, rounded up to nearest 1,000
    - _Requirements: 2.1, 2.2, 2.3, 2.4_
  - [ ] 4.2 Add the date of the baseline run and the Soroban SDK version
    - _Requirements: 2.5_
  - [ ] 4.3 Add an empty "Optimization Log" section
    - _Requirements: 5.1_

- [ ] 5. Update budget constants in `src/gas_test.rs` with real values
  - Replace `u64::MAX` placeholders with the computed budget values from `docs/gas-budgets.md`
  - _Requirements: 3.3_

- [ ] 6. Write regression tests for all core functions
  - [ ] 6.1 `regression_vouch_typical` — assert CPU ≤ `CPU_BUDGET_VOUCH_TYPICAL` and MEM ≤ `MEM_BUDGET_VOUCH_TYPICAL`
    - Failure message: `"vouch [typical] CPU regression: {measured} > {budget}"`
    - _Requirements: 3.1, 3.2_
  - [ ] 6.2 `regression_vouch_worst`
    - _Requirements: 3.1_
  - [ ] 6.3 `regression_request_loan_typical` and `regression_request_loan_worst`
    - _Requirements: 3.1_
  - [ ] 6.4 `regression_repay_typical` and `regression_repay_worst`
    - _Requirements: 3.1_
  - [ ] 6.5 `regression_slash_typical` and `regression_slash_worst`
    - _Requirements: 3.1_
  - [ ] 6.6 `regression_auto_slash_typical` and `regression_auto_slash_worst`
    - _Requirements: 3.1_
  - [ ] 6.7 `regression_withdraw_vouch_typical`
    - _Requirements: 3.1_
  - [ ] 6.8 `regression_batch_vouch_typical` and `regression_batch_vouch_worst`
    - _Requirements: 3.1_

- [ ] 7. Checkpoint — verify all regression tests pass on a clean run
  - Run `cargo test gas::regression`
  - Confirm all regression tests pass
  - Run `cargo test` to confirm all existing tests still pass
  - Ask the user if questions arise.

- [ ] 8. Identify and implement at least one concrete optimization
  - [ ] 8.1 Profile the worst-case `repay` or `slash` measurement to identify the highest-cost operation
    - Candidate: cache `config(&env)` before the voucher loop instead of calling it inside the loop
    - Candidate: single-pass voucher iteration in `repay` (accumulate total stake and distribute in one loop)
    - _Requirements: 4.1_
  - [ ] 8.2 Implement the chosen optimization in the relevant source file (`src/lib.rs`, `src/vouch.rs`, or `src/governance.rs`)
    - _Requirements: 4.1, 4.3_
  - [ ] 8.3 Run `cargo test` to confirm all existing tests still pass after the optimization
    - _Requirements: 4.3_
  - [ ] 8.4 Re-run the affected measurement tests and record before/after values
    - _Requirements: 4.2_
  - [ ] 8.5 Confirm the optimization achieves ≥ 5% CPU reduction in the worst-case scenario
    - _Requirements: 4.4_

- [ ] 9. Update `docs/gas-budgets.md` with post-optimization measurements
  - [ ] 9.1 Update the budget table with post-optimization values (budget = new baseline × 1.5)
    - _Requirements: 4.5, 2.5_
  - [ ] 9.2 Add an entry to the Optimization Log with: function(s) affected, description, before/after CPU and memory, percentage reduction
    - _Requirements: 5.1, 5.2, 5.3_
  - [ ] 9.3 Update budget constants in `src/gas_test.rs` to match the new post-optimization budgets
    - _Requirements: 3.3_

- [ ] 10. Final checkpoint
  - Run `cargo check` — must pass without errors
  - Run `cargo clippy` — address all warnings
  - Run `cargo test` — all tests must pass
  - Run `cargo test gas::regression` — all regression tests must pass
  - Confirm `docs/gas-budgets.md` contains the Optimization Log entry
  - Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- Always use `env.budget().reset_unlimited()` during test setup (voucher minting, contract init) and `env.budget().reset_default()` immediately before the function being measured
- Budget constants should be `const u64` — not `static` or `lazy_static` — for zero-overhead access in tests
- The 1.5× safety margin is a minimum; for functions with high variance (e.g. `batch_vouch`), consider 2×
- `cargo test gas -- --nocapture` runs all gas tests and prints measurement output; `cargo test gas::regression` runs only the regression assertions
