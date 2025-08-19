use edb_engine::EngineConfig;
use tracing::info;

#[test]
fn test_default_config() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = EngineConfig::default();

    assert_eq!(config.rpc_port, 8545);
    assert_eq!(config.etherscan_api_key, None);
    assert_eq!(config.quick, false);
}

#[test]
fn test_config_with_custom_values() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config = EngineConfig {
        rpc_port: 9545,
        etherscan_api_key: Some("test_key".to_string()),
        quick: true,
    };

    assert_eq!(config.rpc_port, 9545);
    assert_eq!(config.etherscan_api_key, Some("test_key".to_string()));
    assert_eq!(config.quick, true);
}

#[test]
fn test_config_clone() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let config =
        EngineConfig { rpc_port: 8080, etherscan_api_key: Some("key".to_string()), quick: false };

    let cloned = config.clone();

    assert_eq!(config.rpc_port, cloned.rpc_port);
    assert_eq!(config.etherscan_api_key, cloned.etherscan_api_key);
    assert_eq!(config.quick, cloned.quick);
}
