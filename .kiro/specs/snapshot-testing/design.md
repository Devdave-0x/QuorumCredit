# Design Document: Snapshot Testing

## Overview

Snapshot testing captures the full observable contract state after a test scenario and serialises it to a JSON file. On subsequent runs the output is compared to the saved file; any diff fails the test. The project already has a `test_snapshots/tests/` directory with hand-authored JSON files. This feature formalises the format, adds a reusable `capture_snapshot` helper, integrates snapshots into the five critical state transitions, and defines a safe update workflow using the `insta` crate.

## Architecture

```mermaid
flowchart TD
    subgraph Test Run
        A[Test function] -->|calls| B[capture_snapshot]
        B -->|reads storage| C[Soroban Env]
        B -->|reads balances| D[Token Client]
        B -->|reads events| E[env.events().all()]
        B -->|returns| F[ContractSnapshot struct]
        F -->|serialise| G[JSON string]
    end
    subgraph Assertion
        G -->|UPDATE_SNAPSHOTS=1| H[Write to test_snapshots/tests/*.json]
        G -->|normal run| I[Read existing .json file]
        I -->|diff| J{Match?}
        J -->|yes| K[Test passes]
        J -->|no| L[Test fails with diff]
    end
```

The `insta` crate provides the assertion backend. `insta::assert_json_snapshot!` handles file I/O, diffing, and the `INSTA_UPDATE` environment variable automatically.

## Components and Interfaces

### `src/snapshot_testing.rs` — Capture Helper

Gated behind `#[cfg(test)]`. Exports `capture_snapshot` and `ContractSnapshot`.

```rust
#[cfg(test)]
pub mod snapshot_testing {
    use soroban_sdk::{Address, Env};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct ContractSnapshot {
        pub schema_version: String,
        pub scenario: String,
        pub ledger_sequence: u32,
        pub ledger_timestamp: u64,
        pub loans: Vec<serde_json::Value>,
        pub vouches: std::collections::BTreeMap<String, serde_json::Value>,
        pub config: serde_json::Value,
        pub token_balances: std::collections::BTreeMap<String, String>,
        pub events: Vec<EventSnapshot>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct EventSnapshot {
        pub topics: Vec<String>,
        pub data: String,
    }

    pub fn capture_snapshot(
        env: &Env,
        scenario: &str,
        addresses: &[Address],
    ) -> ContractSnapshot {
        // Read loans, vouches, config, balances, events from env
        // ...
        ContractSnapshot { /* ... */ }
    }
}
```

### `Cargo.toml` additions

```toml
[dev-dependencies]
insta = { version = "1", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Snapshot files

Stored under `test_snapshots/tests/` with names matching the `insta` convention:
`snapshot_<scenario>.snap` (insta format) or `snapshot_<scenario>.1.json` (existing convention).

To keep consistency with the existing `.json` files, snapshots are written as plain JSON using `insta::assert_json_snapshot!`.

### Update workflow

```makefile
# Makefile
update-snapshots:
    INSTA_UPDATE=always cargo test
```

## Data Models

### `ContractSnapshot` JSON schema

```json
{
  "schema_version": "1",
  "scenario": "vouch_happy_path",
  "ledger_sequence": 100,
  "ledger_timestamp": 1000000,
  "loans": [
    {
      "id": 1,
      "borrower": "GABC...",
      "amount": "500000",
      "amount_repaid": "0",
      "status": "Active",
      "deadline": 1002592000
    }
  ],
  "vouches": {
    "GABC...": [
      { "voucher": "GXYZ...", "stake": "1000000", "token": "GTOKEN..." }
    ]
  },
  "config": { "yield_bps": 200, "slash_bps": 5000 },
  "token_balances": {
    "GABC...": "9500000",
    "GXYZ...": "9000000"
  },
  "events": [
    { "topics": ["loan", "created"], "data": "(GABC..., 500000)" }
  ]
}
```

All `i128` values are serialised as strings to avoid JSON integer overflow.

## Correctness Properties

### Property 1: Determinism

For any fixed contract state, `capture_snapshot` SHALL produce byte-identical JSON output on every call. Non-deterministic output (e.g. from HashMap iteration order) would cause spurious failures.

**Validates: Requirements 1.1, 2.3**

### Property 2: Completeness

The snapshot SHALL include every storage key written during the test scenario. A snapshot that omits a storage key cannot detect mutations to that key.

**Validates: Requirements 1.2, 1.3, 1.4, 2.2**

### Property 3: Diff detectability

For any two contract states that differ in at least one field, `capture_snapshot` SHALL produce different JSON output. A snapshot that collapses distinct states to the same JSON cannot detect regressions.

**Validates: Requirements 3.7, 5.2**

### Property 4: Update safety

When `INSTA_UPDATE` is not set, `assert_snapshot` SHALL never write to disk. Accidental snapshot updates in CI would silently accept regressions.

**Validates: Requirements 4.5**

### Property 5: Round-trip fidelity

A `ContractSnapshot` serialised to JSON and deserialised back SHALL be equal to the original struct. This ensures the comparison logic is symmetric.

**Validates: Requirements 2.8**

## Error Handling

| Scenario | Behaviour |
|---|---|
| Snapshot file missing, `INSTA_UPDATE` not set | Test fails with message: "Snapshot not found. Run with `INSTA_UPDATE=always cargo test` to generate." |
| Snapshot file missing, `INSTA_UPDATE=always` | Snapshot is written; test passes |
| Snapshot diff detected | Test fails with human-readable field-level diff |
| Storage key not found during capture | Field is serialised as `null`; no panic |
| `serde_json` serialisation error | Test panics with a clear message identifying the failing field |

## Testing Strategy

### Unit tests for the capture helper

- Verify `capture_snapshot` returns identical output for two identical environments.
- Verify `capture_snapshot` returns different output when a single storage value changes.
- Verify JSON round-trip: serialise → deserialise → re-serialise produces identical bytes.

### Integration snapshot tests

One test per critical state transition (vouch, request_loan, repay, slash, auto_slash), each calling `capture_snapshot` and asserting via `insta::assert_json_snapshot!`.

### CI configuration

CI runs `cargo test` without `INSTA_UPDATE`. Any snapshot diff causes a non-zero exit code from `insta`, failing the build.
