# Implementation Plan: Snapshot Testing

## Overview

Additive implementation: add `insta` and `serde_json` to dev-dependencies, create `src/snapshot_testing.rs` with the capture helper, write five integration snapshot tests in `src/snapshot_test.rs`, generate the initial snapshot files, and document the update workflow. No contract logic changes.

## Tasks

- [ ] 1. Add dev-dependencies to `Cargo.toml`
  - Add `insta = { version = "1", features = ["json"] }` under `[dev-dependencies]`
  - Add `serde = { version = "1", features = ["derive"] }` under `[dev-dependencies]`
  - Add `serde_json = "1"` under `[dev-dependencies]`
  - Run `cargo check` to confirm the project compiles
  - _Requirements: 2.1_

- [ ] 2. Create `src/snapshot_testing.rs` — capture helper
  - [ ] 2.1 Define `EventSnapshot` struct with `topics: Vec<String>` and `data: String`, deriving `Serialize` and `Deserialize`
    - _Requirements: 1.6_
  - [ ] 2.2 Define `ContractSnapshot` struct with all fields from the JSON schema in the design doc, deriving `Serialize` and `Deserialize`
    - Use `BTreeMap` (not `HashMap`) for `vouches` and `token_balances` to guarantee deterministic key ordering
    - Serialise all `i128` values as strings
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8_
  - [ ] 2.3 Implement `capture_snapshot(env: &Env, scenario: &str, addresses: &[Address]) -> ContractSnapshot`
    - Read `DataKey::Config` from instance storage
    - Iterate loan IDs via `DataKey::LoanCounter` and read each `DataKey::Loan(id)`
    - Read `DataKey::Vouches(addr)` for each address in the `addresses` slice
    - Query token balance for each address using the token client from `DataKey::Config.token`
    - Collect events via `env.events().all()`
    - Set `ledger_sequence` from `env.ledger().sequence()` and `ledger_timestamp` from `env.ledger().timestamp()`
    - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6_
  - [ ] 2.4 Gate the entire module behind `#[cfg(test)]`
    - _Requirements: 2.7_
  - [ ] 2.5 Run `cargo check` to confirm no compilation errors

- [ ] 3. Checkpoint — verify capture helper compiles and returns valid JSON
  - Write a minimal inline test that calls `capture_snapshot` on a freshly initialised env and serialises the result with `serde_json::to_string_pretty`
  - Confirm the output is valid JSON with all expected top-level keys
  - Run `cargo test` to confirm all existing tests still pass
  - Ask the user if questions arise.

- [ ] 4. Create `src/snapshot_test.rs` — integration snapshot tests
  - [ ] 4.1 Add a shared `setup()` helper (consistent with existing test modules) that initialises the contract, registers the token, and funds the contract
    - _Requirements: 3.1_
  - [ ] 4.2 Write `test_snapshot_vouch` — captures state after a successful `vouch` call
    - Call `capture_snapshot` with the voucher and borrower addresses
    - Assert via `insta::assert_json_snapshot!("snapshot_vouch", &snapshot)`
    - _Requirements: 3.1_
  - [ ] 4.3 Write `test_snapshot_request_loan` — captures state after a successful `request_loan` call
    - Assert via `insta::assert_json_snapshot!("snapshot_request_loan", &snapshot)`
    - _Requirements: 3.2_
  - [ ] 4.4 Write `test_snapshot_repay` — captures state before AND after repayment
    - Capture before: `insta::assert_json_snapshot!("snapshot_repay_before", &before)`
    - Capture after: `insta::assert_json_snapshot!("snapshot_repay_after", &after)`
    - _Requirements: 3.3, 5.3_
  - [ ] 4.5 Write `test_snapshot_slash` — captures state before AND after slash execution
    - Capture before: `insta::assert_json_snapshot!("snapshot_slash_before", &before)`
    - Capture after: `insta::assert_json_snapshot!("snapshot_slash_after", &after)`
    - _Requirements: 3.4, 5.4_
  - [ ] 4.6 Write `test_snapshot_auto_slash` — captures state after `auto_slash` is triggered past the deadline
    - Advance ledger timestamp past `loan.deadline` before calling `auto_slash`
    - Assert via `insta::assert_json_snapshot!("snapshot_auto_slash", &snapshot)`
    - _Requirements: 3.5_

- [ ] 5. Register `snapshot_test` module in `src/lib.rs`
  - Add `#[cfg(test)] mod snapshot_test;` alongside the other test module declarations
  - _Requirements: 5.6_

- [ ] 6. Generate initial snapshot files
  - Run `INSTA_UPDATE=always cargo test snapshot` to write all snapshot files to `test_snapshots/tests/`
  - Inspect each generated JSON file to confirm it contains the expected fields
  - Commit the generated snapshot files
  - _Requirements: 3.6, 4.1_

- [ ] 7. Checkpoint — verify snapshot assertions pass on a clean run
  - Run `cargo test snapshot` WITHOUT `INSTA_UPDATE` set
  - Confirm all snapshot tests pass (no diffs)
  - Run `cargo test` to confirm all existing tests still pass
  - Ask the user if questions arise.

- [ ] 8. Add `Makefile` target and documentation
  - [ ] 8.1 Add `update-snapshots` target to `Makefile`: `INSTA_UPDATE=always cargo test`
    - _Requirements: 4.2_
  - [ ] 8.2 Create `docs/snapshot-testing.md` documenting the update workflow
    - Include: when to update snapshots, how to run `make update-snapshots`, how to review the diff in a PR
    - _Requirements: 4.4_

- [ ]* 9. Add unit tests for the capture helper itself
  - [ ]* 9.1 Write a test verifying `capture_snapshot` produces identical output for two identical environments
    - _Requirements: 5.1_
  - [ ]* 9.2 Write a test verifying `capture_snapshot` produces different output when a single storage value changes
    - _Requirements: 5.2_
  - [ ]* 9.3 Write a test verifying JSON round-trip: serialise → deserialise → re-serialise produces identical bytes
    - _Requirements: 2.8_

- [ ] 10. Final checkpoint
  - Run `cargo check` — must pass without errors
  - Run `cargo clippy` — address all warnings
  - Run `cargo test` — all 23+ tests must pass including all snapshot tests
  - Confirm all snapshot files are committed to `test_snapshots/tests/`
  - Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- `insta` stores snapshots in a `snapshots/` subdirectory by default; configure `snapshot_path` in `Cargo.toml` under `[package.metadata.insta]` to point to `test_snapshots/tests/` to match the existing convention
- `BTreeMap` is essential for deterministic JSON key ordering — never use `HashMap` in `ContractSnapshot`
- All `i128` fields must be serialised as strings; use a custom `serde` helper or `#[serde(with = "serde_i128_string")]`
