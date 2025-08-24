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

use edb_engine::source::SourceApiKeys;
use tracing::{debug, info, warn};

#[test]
fn test_source_api_keys_default() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let keys = SourceApiKeys::default();

    assert_eq!(keys.etherscan, None);
    assert_eq!(keys.arbiscan, None);
    assert_eq!(keys.optimistic_etherscan, None);
    assert_eq!(keys.polygonscan, None);
    assert_eq!(keys.bscscan, None);
}

#[test]
fn test_source_api_keys_from_env() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    // Save current env vars
    let saved_etherscan = std::env::var("ETHERSCAN_API_KEY").ok();

    // Set test values
    std::env::set_var("ETHERSCAN_API_KEY", "test_etherscan_key");

    let keys = SourceApiKeys::from_env();

    assert_eq!(keys.etherscan, Some("test_etherscan_key".to_string()));

    // Restore env vars
    if let Some(val) = saved_etherscan {
        std::env::set_var("ETHERSCAN_API_KEY", val);
    } else {
        std::env::remove_var("ETHERSCAN_API_KEY");
    }
}

#[test]
fn test_source_api_keys_clone() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let keys = SourceApiKeys {
        etherscan: Some("key1".to_string()),
        arbiscan: Some("key2".to_string()),
        optimistic_etherscan: None,
        polygonscan: Some("key3".to_string()),
        bscscan: None,
    };

    let cloned = keys.clone();

    assert_eq!(keys.etherscan, cloned.etherscan);
    assert_eq!(keys.arbiscan, cloned.arbiscan);
    assert_eq!(keys.optimistic_etherscan, cloned.optimistic_etherscan);
    assert_eq!(keys.polygonscan, cloned.polygonscan);
    assert_eq!(keys.bscscan, cloned.bscscan);
}
