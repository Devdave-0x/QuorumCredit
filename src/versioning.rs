//! API versioning support (Issue #723)
//!
//! This module provides utilities for managing and querying the contract's API version.
//! It allows clients to determine compatibility and handle version-specific behavior.

use crate::types::{ApiVersion, DataKey, API_VERSION};
use soroban_sdk::Env;

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

/// Check if the contract supports a specific API version.
/// Returns true if the requested version is compatible with the current version.
pub fn is_version_compatible(requested: (u32, u32, u32), current: (u32, u32, u32)) -> bool {
    // Major version must match for compatibility
    if requested.0 != current.0 {
        return false;
    }
    // Minor version of current must be >= requested minor version
    if current.1 < requested.1 {
        return false;
    }
    true
}

/// Get the API version as a semantic version string.
pub fn get_version_string(env: &Env) -> soroban_sdk::String {
    let version = get_api_version(env);
    let version_str = format!(
        "{}.{}.{}",
        version.major, version.minor, version.patch
    );
    soroban_sdk::String::from_slice(env, version_str.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compatibility() {
        // Same version
        assert!(is_version_compatible((1, 0, 0), (1, 0, 0)));

        // Current is newer minor
        assert!(is_version_compatible((1, 0, 0), (1, 1, 0)));

        // Current is newer patch
        assert!(is_version_compatible((1, 0, 0), (1, 0, 1)));

        // Different major version
        assert!(!is_version_compatible((1, 0, 0), (2, 0, 0)));

        // Requested is newer minor
        assert!(!is_version_compatible((1, 1, 0), (1, 0, 0)));
    }
}
