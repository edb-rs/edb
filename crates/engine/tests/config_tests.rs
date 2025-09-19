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

//! Integration tests for EDB engine configuration management.
//!
//! This test suite validates the configuration system of the EDB debugging engine,
//! ensuring that default values are correctly set and custom configurations work as expected.
//! The tests cover configuration creation, cloning behavior, and proper handling of various
//! configuration parameters including RPC proxy URLs, Etherscan API keys, and execution modes.

use edb_engine::EngineConfig;
use tracing::info;

#[test]
fn test_default_config() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = EngineConfig::default();

    assert_eq!(config.rpc_proxy_url, "http://localhost:8545");
    assert_eq!(config.etherscan_api_key, None);
    assert_eq!(config.quick, false);
}

#[test]
fn test_config_with_custom_values() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = EngineConfig {
        rpc_proxy_url: "http://localhost:9545".to_string(),
        etherscan_api_key: Some("test_key".to_string()),
        quick: true,
    };

    assert_eq!(config.rpc_proxy_url, "http://localhost:9545");
    assert_eq!(config.etherscan_api_key, Some("test_key".to_string()));
    assert_eq!(config.quick, true);
}

#[test]
fn test_config_clone() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = EngineConfig {
        rpc_proxy_url: "http://localhost:8080".to_string(),
        etherscan_api_key: Some("key".to_string()),
        quick: false,
    };

    let cloned = config.clone();

    assert_eq!(config.rpc_proxy_url, cloned.rpc_proxy_url);
    assert_eq!(config.etherscan_api_key, cloned.etherscan_api_key);
    assert_eq!(config.quick, cloned.quick);
}
