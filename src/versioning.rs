//! Contract versioning — Issues #742, #743, #744
//!
//! Provides semantic versioning for the QuorumCredit contract, on-chain
//! deployment record-keeping, and rollback snapshot management.

use crate::types::{
    ApiVersion, Config, ContractSemVer, DataKey, DeploymentRecord, RollbackSnapshot,
    VersionHistoryEntry, API_VERSION,
};
use soroban_sdk::{Address, Env, String};

// ── API Version helpers (legacy, Issue #723) ──────────────────────────────────

/// Initialize the API version on contract deployment.
pub fn initialize_api_version(env: &Env) {
    let version = ApiVersion {
        major: 1,
        minor: 0,
        patch: 0,
    };
    env.storage().instance().set(&DataKey::ApiVersion, &version);
}

/// Get the current API version.
pub fn get_api_version(env: &Env) -> ApiVersion {
    env.storage()
        .instance()
        .get::<DataKey, ApiVersion>(&DataKey::ApiVersion)
        .unwrap_or(ApiVersion {
            major: 1,
            minor: 0,
            patch: 0,
        })
}

/// Returns true when `requested` is compatible with `current`.
/// Major versions must match; current minor must be ≥ requested minor.
pub fn is_version_compatible(requested: (u32, u32, u32), current: (u32, u32, u32)) -> bool {
    if requested.0 != current.0 {
        return false;
    }
    current.1 >= requested.1
}

/// Get the API version as a "major.minor.patch" string.
pub fn get_version_string(env: &Env) -> String {
    let v = get_api_version(env);
    let s = format!("{}.{}.{}", v.major, v.minor, v.patch);
    String::from_slice(env, s.as_str())
}

// ── Semantic Contract Versioning (Issue #742) ─────────────────────────────────

/// Return the current semantic contract version, defaulting to 1.0.0.
pub fn get_contract_version(env: &Env) -> ContractSemVer {
    env.storage()
        .instance()
        .get::<DataKey, ContractSemVer>(&DataKey::ContractVersion)
        .unwrap_or_else(|| ContractSemVer {
            major: 1,
            minor: 0,
            patch: 0,
            updated_at: 0,
            note: String::from_slice(env, "initial"),
        })
}

/// Set a new semantic contract version and append it to the on-chain history log.
///
/// * `major` / `minor` / `patch` — new version numbers
/// * `note` — short change description (max 64 chars recommended)
pub fn set_contract_version(env: &Env, major: u32, minor: u32, patch: u32, note: String) {
    let new_ver = ContractSemVer {
        major,
        minor,
        patch,
        updated_at: env.ledger().timestamp(),
        note,
    };
    env.storage()
        .instance()
        .set(&DataKey::ContractVersion, &new_ver);

    let count: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::ContractVersionHistoryCount)
        .unwrap_or(0);

    let entry = VersionHistoryEntry {
        version: new_ver,
        index: count,
    };
    env.storage()
        .instance()
        .set(&DataKey::ContractVersionHistory(count), &entry);
    env.storage()
        .instance()
        .set(&DataKey::ContractVersionHistoryCount, &(count + 1));
}

/// Bump the patch component and record the change.
pub fn bump_patch(env: &Env, note: String) {
    let v = get_contract_version(env);
    set_contract_version(env, v.major, v.minor, v.patch + 1, note);
}

/// Bump the minor component (resets patch to 0) and record the change.
pub fn bump_minor(env: &Env, note: String) {
    let v = get_contract_version(env);
    set_contract_version(env, v.major, v.minor + 1, 0, note);
}

/// Bump the major component (resets minor and patch to 0) and record the change.
pub fn bump_major(env: &Env, note: String) {
    let v = get_contract_version(env);
    set_contract_version(env, v.major + 1, 0, 0, note);
}

/// Retrieve a specific version history entry by its sequential index.
pub fn get_version_history_entry(env: &Env, index: u32) -> Option<VersionHistoryEntry> {
    env.storage()
        .instance()
        .get::<DataKey, VersionHistoryEntry>(&DataKey::ContractVersionHistory(index))
}

/// Total number of recorded version history entries.
pub fn version_history_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::ContractVersionHistoryCount)
        .unwrap_or(0)
}

// ── Deployment Records (Issue #743) ──────────────────────────────────────────

/// Record a new deployment on-chain, advancing the deployment counter.
///
/// Call this after a successful `initialize` or upgrade so that every
/// deployment is tracked with its deployer, timestamp, version, and network.
pub fn record_deployment(env: &Env, deployer: Address, network: String) {
    let count: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::DeploymentRecordCount)
        .unwrap_or(0);

    let record = DeploymentRecord {
        index: count,
        deployer,
        deployed_at: env.ledger().timestamp(),
        version: get_contract_version(env),
        network,
    };

    env.storage()
        .instance()
        .set(&DataKey::DeploymentRecord(count), &record);
    env.storage()
        .instance()
        .set(&DataKey::DeploymentRecordCount, &(count + 1));
}

/// Retrieve a deployment record by index.
pub fn get_deployment_record(env: &Env, index: u32) -> Option<DeploymentRecord> {
    env.storage()
        .instance()
        .get::<DataKey, DeploymentRecord>(&DataKey::DeploymentRecord(index))
}

/// Total number of recorded deployments.
pub fn deployment_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::DeploymentRecordCount)
        .unwrap_or(0)
}

// ── Rollback Snapshots (Issue #744) ──────────────────────────────────────────

/// Save a rollback snapshot for the current deployment index.
///
/// Call this *before* applying any upgrade so that the previous state is
/// preserved and can be restored via `apply_rollback_snapshot`.
pub fn save_rollback_snapshot(
    env: &Env,
    deployment_index: u32,
    yield_bps: i128,
    slash_bps: i128,
    max_vouchers: u32,
    admin_threshold: u32,
) {
    let snapshot = RollbackSnapshot {
        deployment_index,
        snapshot_at: env.ledger().timestamp(),
        version: get_contract_version(env),
        yield_bps,
        slash_bps,
        max_vouchers,
        admin_threshold,
    };
    env.storage()
        .instance()
        .set(&DataKey::RollbackSnapshot(deployment_index), &snapshot);
}

/// Retrieve a rollback snapshot by deployment index.
pub fn get_rollback_snapshot(env: &Env, deployment_index: u32) -> Option<RollbackSnapshot> {
    env.storage()
        .instance()
        .get::<DataKey, RollbackSnapshot>(&DataKey::RollbackSnapshot(deployment_index))
}

/// Returns `true` when a rollback snapshot exists for the given index.
pub fn has_rollback_snapshot(env: &Env, deployment_index: u32) -> bool {
    env.storage()
        .instance()
        .has(&DataKey::RollbackSnapshot(deployment_index))
}

/// Restore critical config fields from a saved rollback snapshot.
///
/// Applies `yield_bps`, `slash_bps`, `max_vouchers`, and `admin_threshold`
/// from the snapshot back into the on-chain `Config`. Returns `false` if no
/// snapshot exists for the given index.
pub fn apply_rollback_snapshot(env: &Env, deployment_index: u32) -> bool {
    let snapshot = match get_rollback_snapshot(env, deployment_index) {
        Some(s) => s,
        None => return false,
    };
    let mut cfg: Config = env
        .storage()
        .instance()
        .get(&DataKey::Config)
        .expect("not initialized");
    cfg.yield_bps = snapshot.yield_bps;
    cfg.slash_bps = snapshot.slash_bps;
    cfg.max_vouchers = snapshot.max_vouchers;
    cfg.admin_threshold = snapshot.admin_threshold;
    env.storage().instance().set(&DataKey::Config, &cfg);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compatibility() {
        assert!(is_version_compatible((1, 0, 0), (1, 0, 0)));
        assert!(is_version_compatible((1, 0, 0), (1, 1, 0)));
        assert!(is_version_compatible((1, 0, 0), (1, 0, 1)));
        assert!(!is_version_compatible((1, 0, 0), (2, 0, 0)));
        assert!(!is_version_compatible((1, 1, 0), (1, 0, 0)));
    }
}
