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

//! Environment variable name constants for EDB configuration.
//!
//! This module provides constant string names for all environment variables used by EDB.
//! These constants ensure consistency across the codebase and provide a single source of
//! truth for environment variable names.
//!
//! # Environment Variables
//!
//! ## Runtime Configuration
//! - [`EDB_ASSERT`] - Controls selective runtime assertion macros
//! - [`EDB_CACHE_DIR`] - Specifies the cache directory location
//! - [`EDB_ETHERSCAN_CACHE_TTL`] - Sets Etherscan cache time-to-live
//!
//! ## Testing Configuration
//! - [`EDB_TEST_ETHERSCAN_MODE`] - Controls Etherscan behavior in tests
//! - [`EDB_TEST_PROXY_MODE`] - Controls proxy behavior in tests

/// Environment variable for controlling selective runtime assertions.
///
/// This variable enables fine-grained control over which assertion macros are active
/// at runtime, similar to how `RUST_LOG` controls logging. When set, it determines
/// which modules' assertions will be evaluated.
///
/// # Syntax
///
/// - `EDB_ASSERT=*` or `EDB_ASSERT=all` - Enable all assertions
/// - `EDB_ASSERT=edb_engine` - Enable assertions in the `edb_engine` crate and submodules
/// - `EDB_ASSERT=edb_engine::inspector` - Enable assertions in specific module and children
/// - `EDB_ASSERT=edb_engine::inspector,edb_common::types` - Multiple targets (comma-separated)
///
/// # Default
///
/// When not set or empty, all assertions are **disabled**.
///
/// # Examples
///
/// ```bash
/// # Enable all assertions
/// EDB_ASSERT=* cargo test
///
/// # Enable assertions only in the engine crate
/// EDB_ASSERT=edb_engine cargo run
///
/// # Enable assertions in specific modules
/// EDB_ASSERT=edb_engine::inspector,edb_common::types cargo test
/// ```
///
/// # Related
///
/// See [`crate::macros`] for the assertion macros that use this variable.
pub const EDB_ASSERT: &str = "EDB_ASSERT";

/// Environment variable for specifying the cache directory.
///
/// This variable determines where EDB stores cached data, including:
/// - Compiled contract artifacts
/// - Etherscan API responses
/// - Other persistent cache data
///
/// # Default
///
/// When not set, EDB uses platform-specific default cache locations determined by
/// the [`EdbCachePath`] implementation.
///
/// # Examples
///
/// ```bash
/// # Use a custom cache directory
/// EDB_CACHE_DIR=/tmp/edb-cache cargo run
///
/// # Can also be set via CLI argument
/// edb --cache-dir /tmp/edb-cache replay <tx-hash>
/// ```
///
/// # Related
///
/// This is also available as a CLI argument (`--cache-dir`) which takes precedence
/// over the environment variable.
pub const EDB_CACHE_DIR: &str = "EDB_CACHE_DIR";

/// Environment variable for setting Etherscan cache time-to-live (TTL) in seconds.
///
/// Controls how long Etherscan API responses are cached before being considered stale.
/// This helps reduce API calls and improve performance for frequently accessed contracts.
///
/// # Value Format
///
/// Must be a valid `u64` integer representing seconds. Invalid values are ignored.
///
/// # Default
///
/// When not set, a default TTL is used (implementation-specific).
///
/// # Examples
///
/// ```bash
/// # Cache for 1 hour (3600 seconds)
/// EDB_ETHERSCAN_CACHE_TTL=3600 cargo run
///
/// # Cache effectively forever (used in tests)
/// EDB_ETHERSCAN_CACHE_TTL=4294967295 cargo test
/// ```
pub const EDB_ETHERSCAN_CACHE_TTL: &str = "EDB_ETHERSCAN_CACHE_TTL";

/// Environment variable for controlling Etherscan behavior in tests.
///
/// This **test-only** variable allows tests to run without making real Etherscan API calls
/// by forcing cache-only mode. When set to `"cache-only"`, EDB will only use cached data
/// and skip any on-chain compilation requests.
///
/// # Values
///
/// - `"cache-only"` - Only use cached Etherscan data, skip all API calls
/// - Any other value or unset - Normal Etherscan behavior
///
/// # Examples
///
/// ```bash
/// # Run tests using only cached Etherscan data
/// EDB_TEST_ETHERSCAN_MODE=cache-only cargo test
/// ```
///
/// # Warning
///
/// This variable is intended for testing only and should not be used in production.
pub const EDB_TEST_ETHERSCAN_MODE: &str = "EDB_TEST_ETHERSCAN_MODE";

/// Environment variable for controlling RPC proxy behavior in tests.
///
/// This **test-only** variable allows tests to configure how the RPC proxy operates,
/// particularly useful for integration tests that need to control caching behavior.
///
/// # Values
///
/// - `"cache-only"` - Use cache-only proxy mode (no external RPC calls)
/// - Any other value or unset - Normal proxy mode with caching
///
/// # Examples
///
/// ```bash
/// # Run integration tests with cache-only proxy
/// EDB_TEST_PROXY_MODE=cache-only cargo test -p edb-integration-tests
/// ```
///
/// # Warning
///
/// This variable is intended for testing only and should not be used in production.
pub const EDB_TEST_PROXY_MODE: &str = "EDB_TEST_PROXY_MODE";
