# Requirements Document

## Introduction

This feature adds snapshot testing to the QuorumCredit Soroban smart contract test suite. Snapshot testing captures the full observable contract state — storage values, token balances, and emitted events — after a test scenario executes, serialises that state to a JSON file, and on subsequent runs compares the live output against the saved file. Any unintended difference fails the test immediately. The goal is to catch accidental state regressions across all critical state transitions (vouch, request_loan, repay, slash, auto_slash) without requiring developers to write exhaustive manual assertions for every field.

The existing `test_snapshots/tests/` directory already contains hand-authored JSON files (e.g. `test_vouch_and_loan_disbursed.1.json`). This feature formalises the format, provides a reusable capture helper, integrates snapshots into the key test scenarios, and defines a safe update workflow for when behaviour intentionally changes.

## Glossary

- **Snapshot**: A JSON file that records the full observable contract state at a specific point in a test scenario.
- **Snapshot File**: A `.json` file stored under `test_snapshots/tests/` whose name encodes the test name and a sequence number (e.g. `test_vouch_and_loan_disbursed.1.json`).
- **Capture Helper**: A Rust test-utility function (`capture_snapshot`) that collects storage values, token balances, and emitted events from a `soroban_sdk::Env` and returns a `ContractSnapshot` struct.
- **ContractSnapshot**: The Rust struct that models the full snapshot payload and serialises to/from JSON.
- **Snapshot Assertion**: A call to `assert_snapshot(name, &snapshot)` that either writes a new snapshot file (first run) or diffs the current snapshot against the saved file and panics on any difference.
- **Update Mode**: A test run where `INSTA_UPDATE=always` (or `--update-snapshots` flag) is set, causing `assert_snapshot` to overwrite existing files rather than diff them.
- **State Transition**: One of the five critical contract operations: `vouch`, `request_loan`, `repay`, `slash`, `auto_slash`.
- **LoanRecord**: The `LoanRecord` struct from `src/types.rs` representing a single loan.
- **VouchRecord**: The `VouchRecord` struct from `src/types.rs` representing a single vouch.
- **Config**: The `Config` struct from `src/types.rs` holding all protocol parameters.
- **insta**: The [`insta`](https://insta.rs) Rust snapshot-testing crate used as the assertion backend.

---

## Requirements

### Requirement 1: Define Snapshot Format

**User Story:** As a developer, I want a well-defined JSON schema for contract snapshots so that snapshot files are human-readable, diffable in pull requests, and stable across Rust toolchain upgrades.

#### Acceptance Criteria

1. THE snapshot format SHALL be a JSON object with the following top-level keys: `"schema_version"` (string), `"scenario"` (string), `"ledger_sequence"` (u32), `"ledger_timestamp"` (u64), `"loans"` (array), `"vouches"` (object), `"config"` (object), `"token_balances"` (object), `"events"` (array).
2. WHEN a `LoanRecord` is serialised, THE snapshot SHALL include all fields defined in `src/types.rs` including `id`, `borrower`, `amount`, `amount_repaid`, `total_yield`, `status`, `created_at`, `disbursement_timestamp`, `repayment_timestamp`, `deadline`, `loan_purpose`, `token_address`, and `escrow_status`.
3. WHEN a `VouchRecord` is serialised, THE snapshot SHALL include all fields: `voucher`, `stake`, `vouch_timestamp`, `token`, `expiry_timestamp`, `delegate`, and `chain_id`.
4. WHEN a `Config` is serialised, THE snapshot SHALL include all fields from the `Config` struct in `src/types.rs`.
5. THE `"token_balances"` field SHALL be a JSON object mapping address strings to i128 balance strings (using string encoding to avoid JSON integer overflow on large stroop values).
6. THE `"events"` field SHALL be a JSON array of objects, each with `"topics"` (array of strings) and `"data"` (string) fields representing the emitted contract events.
7. THE `"schema_version"` field SHALL be set to `"1"` for all snapshots produced by this implementation.
8. WHEN a field value is `None` in Rust, THE snapshot SHALL serialise it as JSON `null`.

---

### Requirement 2: Implement Snapshot Capture Helper

**User Story:** As a test author, I want a single `capture_snapshot` function I can call after any test scenario so that I do not have to manually read every storage key and balance in each test.

#### Acceptance Criteria

1. THE Contract test utilities SHALL expose a public function `capture_snapshot(env: &Env, scenario: &str, addresses: &[Address]) -> ContractSnapshot`.
2. WHEN `capture_snapshot` is called, THE function SHALL read all `DataKey::Loan(id)` entries for every loan ID up to `DataKey::LoanCounter`, and include them in the `loans` array.
3. WHEN `capture_snapshot` is called, THE function SHALL read all `DataKey::Vouches(borrower)` entries for every address in the `addresses` slice, and include them in the `vouches` object keyed by borrower address string.
4. WHEN `capture_snapshot` is called, THE function SHALL read `DataKey::Config` and include it in the `config` field.
5. WHEN `capture_snapshot` is called, THE function SHALL query the token balance for each address in the `addresses` slice using the token contract client, and include them in the `token_balances` object.
6. WHEN `capture_snapshot` is called, THE function SHALL collect all events emitted during the test via `env.events().all()` and include them in the `events` array.
7. THE `capture_snapshot` function SHALL be located in a new file `src/snapshot_testing.rs` and gated behind `#[cfg(test)]`.
8. THE `ContractSnapshot` struct SHALL derive `serde::Serialize` and `serde::Deserialize` so it can be round-tripped through JSON without loss.

---

### Requirement 3: Add Snapshot Assertions to Key Test Scenarios

**User Story:** As a developer, I want snapshot assertions embedded in the key test scenarios so that any unintended state change in vouch, request_loan, repay, slash, or auto_slash is caught automatically on the next CI run.

#### Acceptance Criteria

1. THE test suite SHALL include a snapshot test for the `vouch` state transition that captures state after a successful `vouch` call and asserts it against `test_snapshots/tests/snapshot_vouch.1.json`.
2. THE test suite SHALL include a snapshot test for the `request_loan` state transition that captures state after a successful `request_loan` call and asserts it against `test_snapshots/tests/snapshot_request_loan.1.json`.
3. THE test suite SHALL include a snapshot test for the `repay` state transition that captures state after a successful full repayment and asserts it against `test_snapshots/tests/snapshot_repay.1.json`.
4. THE test suite SHALL include a snapshot test for the `slash` state transition that captures state after a successful slash execution and asserts it against `test_snapshots/tests/snapshot_slash.1.json`.
5. THE test suite SHALL include a snapshot test for the `auto_slash` state transition that captures state after an auto-slash triggered by governance quorum and asserts it against `test_snapshots/tests/snapshot_auto_slash.1.json`.
6. WHEN a snapshot test runs for the first time and no snapshot file exists, THE test SHALL write the current state to the snapshot file and pass.
7. WHEN a snapshot test runs and a snapshot file already exists, THE test SHALL compare the current state to the saved file field-by-field and fail with a human-readable diff if any field differs.
8. WHEN a snapshot test fails due to a diff, THE error message SHALL identify the snapshot file path and the first differing field.

---

### Requirement 4: Snapshot Update Workflow

**User Story:** As a developer, I want a documented and automated way to regenerate snapshot files when contract behaviour intentionally changes so that updating snapshots is a deliberate, reviewable action rather than an accidental side-effect.

#### Acceptance Criteria

1. WHEN the environment variable `INSTA_UPDATE=always` is set, THE snapshot assertion SHALL overwrite the existing snapshot file with the current state instead of diffing.
2. THE `Makefile` SHALL include a `update-snapshots` target that runs `INSTA_UPDATE=always cargo test` so developers can regenerate all snapshots with a single command.
3. WHEN snapshots are updated, THE updated JSON files SHALL be committed to version control alongside the code change that caused the behavioural difference.
4. THE `README` or a dedicated `SNAPSHOT_TESTING.md` file SHALL document the update workflow: when to update, how to run `make update-snapshots`, and how to review the diff in a pull request.
5. WHEN `INSTA_UPDATE` is not set (normal CI run), THE snapshot assertion SHALL never modify snapshot files on disk.
6. THE snapshot update mechanism SHALL support a `--update-snapshots` flag passed to `cargo test` as an alternative to the environment variable, implemented via a `SNAPSHOT_UPDATE` feature flag or a runtime check.

---

### Requirement 5: Test Coverage for Critical State Transitions

**User Story:** As a protocol auditor, I want snapshot coverage for every critical state transition so that no storage mutation can go undetected between releases.

#### Acceptance Criteria

1. THE snapshot test suite SHALL cover all five critical state transitions: `vouch`, `request_loan`, `repay`, `slash`, and `auto_slash`.
2. WHEN a new state transition is added to the contract, THE developer SHALL add a corresponding snapshot test before the feature is merged.
3. THE snapshot test for `repay` SHALL capture state both before and after repayment, producing two snapshot files (`snapshot_repay_before.1.json` and `snapshot_repay_after.1.json`), so that the delta is visible in the diff.
4. THE snapshot test for `slash` SHALL capture state both before and after the slash, producing two snapshot files (`snapshot_slash_before.1.json` and `snapshot_slash_after.1.json`).
5. THE CI pipeline SHALL run all snapshot tests on every pull request and fail if any snapshot diff is detected.
6. THE snapshot test module SHALL be registered in `src/tests.rs` (or `src/lib.rs`) alongside the other test modules so it is included in `cargo test` runs.
