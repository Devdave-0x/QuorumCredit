# Requirements Document

## Introduction

This feature adds gas optimization tests to the QuorumCredit Soroban smart contract. In Soroban, "gas" is measured as CPU instructions and memory bytes consumed during contract execution. The Soroban SDK exposes these metrics through the `env.budget()` API: `env.budget().reset_default()` resets the meter before a call, and `env.budget().cpu_instruction_count()` / `env.budget().mem_bytes_used()` read the accumulated totals after a call.

The goal is to measure the resource consumption of each core contract function, document per-function upper bounds (budgets) in `docs/gas-budgets.md`, write regression tests that fail if any function exceeds its documented budget, and identify and implement at least one concrete optimization that reduces consumption. This ensures that future code changes do not silently degrade performance and that the contract remains within Soroban's execution limits.

## Glossary

- **Contract**: The `QuorumCreditContract` Soroban smart contract.
- **Gas**: The combined resource cost of a contract invocation, measured as CPU instruction count and memory bytes used.
- **CPU Instruction Count**: The number of CPU instructions consumed by a contract call, read via `env.budget().cpu_instruction_count()`.
- **Memory Bytes Used**: The number of bytes of memory allocated during a contract call, read via `env.budget().mem_bytes_used()`.
- **Budget API**: The Soroban SDK interface `env.budget()` that exposes `reset_default()`, `cpu_instruction_count()`, and `mem_bytes_used()`.
- **Gas Budget**: A documented upper bound (ceiling) for the CPU instructions and memory bytes a specific function may consume. Stored in `docs/gas-budgets.md`.
- **Gas Regression Test**: A test that measures a function's resource consumption and asserts it does not exceed the documented gas budget.
- **Typical-Case Scenario**: A measurement taken with a single voucher backing a borrower — the minimum realistic configuration.
- **Worst-Case Scenario**: A measurement taken with the maximum number of vouchers (`DEFAULT_MAX_VOUCHERS_PER_BORROWER`, currently 50) backing a borrower — the configuration that exercises all linear-scan loops.
- **Gas Test Module**: The `src/gas_test.rs` test module that contains all measurement and regression tests.
- **Optimization**: A code change that reduces CPU instruction count or memory bytes used for one or more functions without altering observable behaviour.

---

## Requirements

### Requirement 1: Measure CPU and Memory Consumption for Core Functions

**User Story:** As a protocol developer, I want to measure the CPU instruction count and memory bytes used by each core contract function so that I have a baseline for optimization and can detect regressions.

#### Acceptance Criteria

1. THE test suite SHALL measure CPU instruction count and memory bytes used for each of the following functions: `vouch`, `request_loan`, `repay`, `slash` (via `vote_slash` + `execute_pending_slash`), `auto_slash`, `withdraw_vouch`, and `batch_vouch`.
2. WHEN measuring a function, THE test SHALL call `env.budget().reset_default()` immediately before the function under test and read `env.budget().cpu_instruction_count()` and `env.budget().mem_bytes_used()` immediately after.
3. THE test suite SHALL measure each function under a typical-case scenario (one voucher) AND a worst-case scenario (maximum vouchers, currently `DEFAULT_MAX_VOUCHERS_PER_BORROWER = 50`).
4. WHEN a measurement is taken, THE test SHALL print the function name, scenario label, CPU instruction count, and memory bytes used to standard output so that results are visible in `cargo test -- --nocapture`.
5. THE test suite SHALL be located in `src/gas_test.rs` and registered in `src/lib.rs` under `#[cfg(test)]`.

---

### Requirement 2: Document Gas Budgets

**User Story:** As a protocol developer, I want per-function gas budgets documented in a single file so that reviewers and CI can verify that no function silently exceeds its resource limits.

#### Acceptance Criteria

1. THE project SHALL contain a file `docs/gas-budgets.md` that documents the gas budget for each measured function.
2. THE `docs/gas-budgets.md` file SHALL contain a table with columns: Function, Scenario, CPU Budget (instructions), Memory Budget (bytes), and Notes.
3. WHEN a budget is set, THE budget SHALL be derived from the measured baseline value multiplied by a safety margin of 1.5× (i.e., budget = measured × 1.5, rounded up to the nearest 1,000 instructions / 1,000 bytes).
4. THE `docs/gas-budgets.md` file SHALL document budgets for both typical-case and worst-case scenarios for each function listed in Requirement 1.1.
5. WHEN the `docs/gas-budgets.md` file is updated, THE file SHALL include the date of the last measurement and the Soroban SDK version used.

---

### Requirement 3: Gas Regression Tests

**User Story:** As a CI engineer, I want automated tests that fail if any contract function exceeds its documented gas budget so that performance regressions are caught before deployment.

#### Acceptance Criteria

1. THE test suite SHALL contain one regression test per function per scenario (typical-case and worst-case) that asserts CPU instruction count ≤ documented CPU budget AND memory bytes used ≤ documented memory budget.
2. WHEN a regression test fails, THE failure message SHALL include the function name, scenario, measured value, and budget value so that the developer can identify the regression.
3. THE regression tests SHALL use the budget constants defined in `src/gas_test.rs` (not hardcoded inline) so that budgets can be updated in a single place.
4. THE regression tests SHALL pass on a clean checkout with no code changes — i.e., the initial budgets SHALL be set conservatively enough to pass on the first run.
5. WHEN `cargo test` is run, ALL gas regression tests SHALL pass without requiring any special flags or environment variables.

---

### Requirement 4: Identify and Implement at Least One Concrete Optimization

**User Story:** As a protocol developer, I want at least one concrete gas optimization implemented so that the contract's resource consumption is demonstrably reduced.

#### Acceptance Criteria

1. THE implementation SHALL identify at least one concrete optimization opportunity in the contract source code (e.g., reducing redundant storage reads, caching a value that is read multiple times in a loop, avoiding redundant iterations over the voucher list).
2. WHEN an optimization is implemented, THE before-and-after CPU instruction counts and memory bytes SHALL be measured and recorded.
3. THE optimization SHALL not alter the observable behaviour of the contract — all existing tests SHALL continue to pass after the optimization is applied.
4. THE optimization SHALL reduce CPU instruction count or memory bytes used by at least 5% for the affected function in the worst-case scenario.
5. WHEN the optimization is applied, THE gas budgets in `docs/gas-budgets.md` SHALL be updated to reflect the post-optimization measurements.

---

### Requirement 5: Document Optimization Findings

**User Story:** As a protocol developer, I want optimization findings and before/after measurements documented so that the rationale for each change is preserved for future maintainers.

#### Acceptance Criteria

1. THE `docs/gas-budgets.md` file SHALL contain a section titled "Optimization Log" that records each optimization applied.
2. WHEN an optimization is recorded, THE entry SHALL include: the function(s) affected, a description of the change, the before measurement (CPU instructions, memory bytes), the after measurement (CPU instructions, memory bytes), and the percentage reduction.
3. THE Optimization Log SHALL include at least one entry corresponding to the optimization implemented in Requirement 4.
4. WHEN a future optimization is applied, THE developer SHALL add a new entry to the Optimization Log rather than overwriting existing entries.

---

### Requirement 6: Test Coverage

**User Story:** As a developer, I want the gas test module to be well-structured and self-contained so that it can be maintained independently of the functional test suite.

#### Acceptance Criteria

1. THE `src/gas_test.rs` module SHALL contain a shared `setup()` helper that initialises the contract environment, registers the token, and funds the contract — consistent with the pattern used in existing test modules.
2. THE test module SHALL define budget constants as `const` values at the top of the module so that they are easy to locate and update.
3. THE test module SHALL separate measurement tests (which print results) from regression tests (which assert bounds) so that measurement tests can be run independently with `-- --nocapture`.
4. THE test module SHALL include a comment at the top explaining how to run the measurement tests and how to update budgets.
5. WHEN `cargo test gas` is run, ALL tests in `src/gas_test.rs` SHALL pass.
