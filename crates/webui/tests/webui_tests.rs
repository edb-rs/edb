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

use edb_webui::WebUiConfig;
use tracing::{debug, info, warn};

#[test]
fn test_default_webui_config() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = WebUiConfig::default();

    assert_eq!(config.port, 3000);
    assert_eq!(config.engine_rpc_url, "http://localhost:8545");
}

#[test]
fn test_custom_webui_config() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = WebUiConfig { port: 8080, engine_rpc_url: "http://localhost:9545".to_string() };

    assert_eq!(config.port, 8080);
    assert_eq!(config.engine_rpc_url, "http://localhost:9545");
}

#[test]
fn test_webui_config_clone() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = WebUiConfig { port: 4000, engine_rpc_url: "http://test:8545".to_string() };

    let cloned = config.clone();

    assert_eq!(config.port, cloned.port);
    assert_eq!(config.engine_rpc_url, cloned.engine_rpc_url);
}
