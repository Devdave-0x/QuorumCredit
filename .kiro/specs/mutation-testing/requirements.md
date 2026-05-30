# Requirements Document

## Introduction

This feature introduces mutation testing to the QuorumCredit Soroban smart contract project using the `cargo-mutants` tool. Mutation testing systematically injects small code changes (mutations) into the source and verifies that the existing test suite detects them. A high mutation kill rate provides confidence that the tests are meaningful and that the contract logic is well-guarded against regressions. The goal is to establish a baseline kill-rate score, enforce a minimum threshold, and document any surviving mutants so they can be addressed with targeted tests or accepted as low-risk.

## Glossary

- **Mutation**: A small, syntactically valid change to the source code introduced by `cargo-mutants` (e.g. replacing `+` with `-`, changing a comparison operator, returning a constant instead of a computed value).
- **Mutant**: A single version of the source code that contains exactly one mutation.
- **Killed Mutant**: A mutant for which at least one test in the suite fails, indicating the test suite detected the change.
- **Surviving Mutant**: A mutant for which all tests pass, indicating the test suite did not detect the change.
- **Kill Rate**: The ratio of killed mutants to total mutants, expressed as a percentage. `kill_rate = killed / total * 100`.
- **Mutation Score**: Synonym for Kill Rate in this document.
- **Threshold**: The minimum acceptable Kill Rate below which the mutation testing run is considered a failure.
- **Scope**: The set of source files and functions submitted to `cargo-mutants` for mutation.
- **cargo-mutants**: The Rust mutation testing tool (`cargo install cargo-mutants`) that drives the mutation process.
- **Baseline Run**: The first execution of `cargo-mutants` against the project, establishing the initial Kill Rate before any test improvements.

---

## Requirements

### Requirement 1: Install and Configure cargo-mutants

**User Story:** As a developer, I want `cargo-mutants` installed and configured for the QuorumCredit project so that I can run mutation testing locally and in CI without manual setup steps.

#### Acceptance Criteria

1. WHEN a developer runs `cargo install cargo-mutants`, THE tool SHALL install successfully against the project's current Rust toolchain.
2. THE project SHALL include a `mutants.toml` configuration file at the repository root that specifies the mutation scope, timeout per mutant, and any files or functions to exclude.
3. WHEN `cargo mutants --list` is run from the repository root, THE tool SHALL enumerate the mutants in scope without errors.
4. THE `mutants.toml` SHALL exclude generated code, test modules, and any files outside the defined scope so that mutation runs complete in a reasonable time.
5. WHEN `cargo mutants` is run, THE tool SHALL use the project's existing test suite (all modules) to evaluate each mutant without requiring additional configuration.

---

### Requirement 2: Define Mutation Testing Scope

**User Story:** As a developer, I want a clearly defined set of source files and functions in scope for mutation testing so that the tool focuses on contract logic rather than test helpers or generated code.

#### Acceptance Criteria

1. THE mutation scope SHALL include the following source files: `src/lib.rs`, `src/vouch.rs`, `src/governance.rs`, `src/admin.rs`, `src/helpers.rs`.
2. THE mutation scope SHALL NOT include test modules (`#[cfg(test)]` blocks), build scripts, or generated files.
3. WHEN `cargo mutants --list` is run, THE output SHALL list mutants only from the files defined in Acceptance Criterion 2.1.
4. THE `mutants.toml` SHALL document the rationale for each included and excluded file so that future maintainers understand the scope decisions.
5. WHEN a new source file is added to the project, THE developer SHALL update `mutants.toml` to explicitly include or exclude it.

---

### Requirement 3: Establish Mutation Score Baseline and Threshold

**User Story:** As a developer, I want a documented baseline Kill Rate and a minimum threshold so that I can measure test quality objectively and enforce a quality gate.

#### Acceptance Criteria

1. WHEN `cargo mutants` is run for the first time against the defined scope, THE developer SHALL record the resulting Kill Rate as the baseline in `docs/mutation-testing.md`.
2. THE project SHALL enforce a minimum Kill Rate threshold of **80%** (i.e. at least 80% of generated mutants must be killed by the test suite).
3. WHEN the Kill Rate falls below 80%, THE developer SHALL treat the run as a failure and investigate surviving mutants before merging.
4. THE baseline document SHALL include: total mutants generated, killed count, surviving count, Kill Rate percentage, and the `cargo-mutants` version used.
5. WHEN the test suite is improved to kill previously surviving mutants, THE developer SHALL update the baseline document to reflect the new Kill Rate.

---

### Requirement 4: Integrate Mutation Testing into CI

**User Story:** As a developer, I want mutation testing to run automatically in CI so that regressions in test quality are caught before code is merged.

#### Acceptance Criteria

1. THE CI pipeline SHALL include a dedicated mutation testing job that runs `cargo mutants` against the defined scope.
2. WHEN the mutation testing CI job runs, IT SHALL fail the build if the Kill Rate falls below the 80% threshold defined in Requirement 3.
3. THE CI job SHALL cache the `cargo-mutants` binary and the compiled test artefacts to reduce run time.
4. THE CI job SHALL run on pull requests targeting the main branch and on pushes to the main branch.
5. WHEN the mutation testing job exceeds a configurable time limit (default: 30 minutes), IT SHALL be cancelled and the build SHALL be marked as failed with a timeout message.

---

### Requirement 5: Document Surviving Mutants and Remediation

**User Story:** As a developer, I want surviving mutants documented with a remediation decision so that the team understands which gaps exist in the test suite and why they are or are not addressed.

#### Acceptance Criteria

1. WHEN a mutation testing run completes, THE developer SHALL review all surviving mutants and record each one in `docs/mutation-testing.md`.
2. FOR each surviving mutant, THE document SHALL include: the file and line number, the original code, the mutated code, and a remediation decision (add test / accept as low-risk / exclude from scope).
3. WHEN a surviving mutant is accepted as low-risk, THE developer SHALL add a comment in `mutants.toml` explaining why it is excluded.
4. WHEN a surviving mutant requires a new test, THE developer SHALL write the test and verify it kills the mutant before closing the issue.
5. THE `docs/mutation-testing.md` document SHALL be updated after every mutation testing run that changes the Kill Rate by more than 2 percentage points.

