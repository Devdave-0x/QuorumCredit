# Implementation Plan: Mutation Testing

## Overview

Add `cargo-mutants` mutation testing to the QuorumCredit project. The work is purely additive: no contract logic changes. Deliverables are a `mutants.toml` configuration file, a `docs/mutation-testing.md` baseline report, targeted tests for any surviving mutants above the 80% threshold, and an optional CI workflow.

## Tasks

- [ ] 1. Install `cargo-mutants` and verify it runs against the project
  - [ ] 1.1 Install the tool locally
    - Run `cargo install cargo-mutants --locked`
    - Confirm `cargo mutants --version` prints a version string
    - _Requirements: 1.1_
  - [ ] 1.2 Run a quick smoke test
    - Run `cargo mutants --list` from the repository root (no config yet)
    - Confirm the tool enumerates mutants without compilation errors
    - Note the approximate total mutant count for scoping purposes
    - _Requirements: 1.3_

- [ ] 2. Create `mutants.toml` configuration file at the repository root
  - [ ] 2.1 Define the mutation scope
    - Set `include_globs` to `["src/lib.rs", "src/vouch.rs", "src/governance.rs", "src/admin.rs", "src/helpers.rs"]`
    - _Requirements: 2.1, 2.3_
  - [ ] 2.2 Exclude test modules and generated files
    - Set `exclude_globs` to exclude `src/*_test.rs`, `src/tests/**`, and `build.rs`
    - _Requirements: 2.2, 2.3_
  - [ ] 2.3 Set per-mutant timeout
    - Set `timeout = 60` (seconds) to prevent hanging mutants from blocking the run
    - _Requirements: 1.5_
  - [ ] 2.4 Add rationale comments for each include/exclude decision
    - Document why each file is in or out of scope as inline comments in `mutants.toml`
    - _Requirements: 2.4_
  - [ ] 2.5 Verify scope with `--list`
    - Run `cargo mutants --list` and confirm only the five in-scope files appear
    - Confirm no `*_test.rs` files appear in the output
    - _Requirements: 2.3, 2.2_

- [ ] 3. Checkpoint — verify configuration is correct before running full mutation suite
  - Run `cargo check` to confirm the project still compiles cleanly
  - Run `cargo mutants --list` and confirm the scope matches `mutants.toml`
  - Ask the user if questions arise before proceeding to the full run.

- [ ] 4. Run the baseline mutation testing pass and record results
  - [ ] 4.1 Execute the full mutation testing run
    - Run `cargo mutants` from the repository root
    - Allow the run to complete (estimated 20–60 minutes depending on machine)
    - _Requirements: 3.1_
  - [ ] 4.2 Record the baseline Kill Rate
    - Note: total mutants, killed, survived, timeouts, Kill Rate percentage, `cargo-mutants` version
    - _Requirements: 3.1, 3.4_
  - [ ] 4.3 Create `docs/mutation-testing.md` with the baseline report
    - Include all fields from Acceptance Criterion 3.4: total, killed, survived, Kill Rate, version
    - Include the date of the baseline run
    - _Requirements: 3.1, 3.4_

- [ ] 5. Review surviving mutants and decide on remediation
  - [ ] 5.1 List all surviving mutants from `mutants.out/`
    - Inspect `mutants.out/outcomes.json` or the per-mutant log files
    - _Requirements: 5.1_
  - [ ] 5.2 Categorise each surviving mutant
    - For each survivor: file, line, original code, mutated code
    - Assign a remediation decision: `add test` / `accept as low-risk` / `exclude from scope`
    - _Requirements: 5.2_
  - [ ] 5.3 Document all surviving mutants in `docs/mutation-testing.md`
    - Add a "Surviving Mutants" section with a table: file, line, original, mutant, decision
    - _Requirements: 5.1, 5.2_
  - [ ] 5.4 Add exclusions for accepted low-risk mutants to `mutants.toml`
    - Use `[[exclude_functions]]` entries with a `reason` comment for each accepted survivor
    - _Requirements: 5.3_

- [ ] 6. Write targeted tests to kill surviving mutants (if Kill Rate < 80%)
  - [ ] 6.1 Identify the highest-impact surviving mutants
    - Prioritise mutants in `src/governance.rs` (quorum checks) and `src/vouch.rs` (stake arithmetic) as these guard the most critical contract invariants
    - _Requirements: 3.2, 3.3_
  - [ ] 6.2 Write new tests in the appropriate existing test modules
    - Add tests to `slash_threshold_voting_test` for governance quorum boundary conditions
    - Add tests to `property_stake_loan_invariants_test` for arithmetic edge cases in `src/helpers.rs`
    - Add tests to `cross_chain_vouch_test` for vouch expiry and stake boundary conditions
    - _Requirements: 3.2, 5.4_
  - [ ] 6.3 Verify each new test kills its target mutant
    - Run `cargo mutants --in-diff <patch>` or re-run the full suite to confirm the Kill Rate has improved
    - _Requirements: 5.4_

- [ ] 7. Checkpoint — verify Kill Rate meets the 80% threshold
  - Re-run `cargo mutants` after adding targeted tests
  - Confirm Kill Rate ≥ 80%
  - Update `docs/mutation-testing.md` with the new Kill Rate
  - Run `cargo test` to confirm all new tests pass
  - Ask the user if questions arise.

- [ ] 8. Add `mutants.out/` to `.gitignore`
  - Append `mutants.out/` to `.gitignore` so the per-run output directory is not committed
  - _Requirements: 1.2_

- [ ]* 9. Add CI workflow for mutation testing
  - [ ]* 9.1 Create `.github/workflows/mutation-testing.yml`
    - Define a job that installs `cargo-mutants`, runs `cargo mutants`, and checks the Kill Rate
    - Set `timeout-minutes: 30` on the job
    - _Requirements: 4.1, 4.4, 4.5_
  - [ ]* 9.2 Add `scripts/check_kill_rate.py`
    - Implement the kill-rate assertion script that parses `mutants.out/outcomes.json` and exits non-zero if Kill Rate < 80%
    - _Requirements: 4.2_
  - [ ]* 9.3 Configure caching in the CI workflow
    - Use `Swatinem/rust-cache` or equivalent to cache compiled artefacts and the `cargo-mutants` binary
    - _Requirements: 4.3_
  - [ ]* 9.4 Trigger the CI job on a test branch and confirm it passes
    - Push a branch, open a draft PR, and verify the mutation testing job runs end-to-end
    - _Requirements: 4.1, 4.4_

- [ ] 10. Final checkpoint — confirm everything is in order
  - Run `cargo check` — must pass without errors
  - Run `cargo test` — all tests must pass
  - Run `cargo mutants --list` — scope must match `mutants.toml`
  - Confirm `docs/mutation-testing.md` is up to date with the final Kill Rate
  - Confirm `mutants.out/` is listed in `.gitignore`
  - Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- The full `cargo mutants` run can take 20–60 minutes; run it when the machine is otherwise idle or in CI
- `cargo-mutants` generates one compiled binary per mutant; ensure sufficient disk space (several GB for a full run)
- If the baseline Kill Rate is already ≥ 80%, Task 6 can be skipped entirely
- Surviving mutants in trivial getter functions (e.g. `get_version`) are good candidates for `[[exclude_functions]]` entries
- The `--jobs` flag (e.g. `cargo mutants --jobs 4`) can parallelise the run on multi-core machines

