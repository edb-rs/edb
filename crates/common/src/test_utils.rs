// EDB - Ethereum Debugger
// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Test utilities for configuring the EDB test environment.
//!
//! Provides helpers for setting up cache directories and environment variables
//! needed for testing, with support for both shared and isolated test environments.

use std::{env, path::PathBuf};

use tracing::info;

/// Get the testdata cache directory root path
pub fn get_testdata_cache_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("Failed to find workspace root")
        .join("testdata")
        .join("cache")
}

/// Sets up the test environment with appropriate cache directory and configuration.
///
/// This function configures the [`EDB_CACHE_DIR`](crate::env::EDB_CACHE_DIR) and
/// [`EDB_ETHERSCAN_CACHE_TTL`](crate::env::EDB_ETHERSCAN_CACHE_TTL) environment
/// variables for testing purposes.
///
/// # Arguments
///
/// * `use_temp` - If `true`, copies the testdata cache to a temporary directory for
///   test isolation. This is useful for tests that may modify cache contents and need
///   to avoid polluting the shared testdata cache. If `false`, uses the shared testdata
///   cache directory directly (read-only usage recommended).
///
/// # Behavior
///
/// - Sets `EDB_CACHE_DIR` to either a temporary copy or the shared testdata cache
/// - Sets `EDB_ETHERSCAN_CACHE_TTL` to `u32::MAX` (effectively infinite) to avoid
///   cache expiration during tests
/// - Creates the cache directory if it doesn't exist
///
/// # Temporary Directory
///
/// When `use_temp=true`, the temporary directory is created in the system's temp
/// location with a random suffix for isolation. The testdata cache is recursively
/// copied to this temporary location. The directory will be automatically cleaned
/// up by the OS.
///
/// # Examples
///
/// ```
/// use edb_common::test_utils::setup_test_environment;
///
/// // Use shared testdata cache (read-only tests)
/// setup_test_environment(false);
///
/// // Use isolated temporary cache (tests that modify cache)
/// setup_test_environment(true);
/// ```
pub fn setup_test_environment(use_temp: bool) {
    let cache_dir = if use_temp {
        // Create a temporary directory with random suffix and copy testdata cache into it
        use rand::Rng;
        let random_suffix: u32 = rand::thread_rng().gen();
        let temp_dir = env::temp_dir().join(format!("edb-test-cache-{random_suffix:08x}"));
        let testdata_cache = get_testdata_cache_root();

        if testdata_cache.exists() {
            copy_dir_all(&testdata_cache, &temp_dir)
                .expect("Failed to copy testdata cache to temp directory");
        } else {
            std::fs::create_dir_all(&temp_dir).expect("Failed to create temp cache directory");
        }

        info!("Using temporary test cache directory: {}", temp_dir.display());
        temp_dir
    } else {
        let cache_dir = get_testdata_cache_root();

        // Create the directory if it doesn't exist
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");
        }

        info!("Using shared test cache directory: {}", cache_dir.display());
        cache_dir
    };

    // Set the environment variables
    env::set_var(crate::env::EDB_CACHE_DIR, cache_dir.to_str().expect("Invalid cache path"));
    env::set_var(crate::env::EDB_ETHERSCAN_CACHE_TTL, format!("{}", u32::MAX));
}

/// Helper function to recursively copy directories.
///
/// This is a simple implementation for copying directory trees, used for test isolation.
fn copy_dir_all(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
