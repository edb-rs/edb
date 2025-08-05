use edb_engine::EngineConfig;

#[test]
fn test_default_config() {
    let config = EngineConfig::default();

    assert_eq!(config.rpc_port, 8545);
    assert_eq!(config.etherscan_api_key, None);
}

#[test]
fn test_config_with_custom_values() {
    let config = EngineConfig {
        rpc_port: 9545,
        etherscan_api_key: Some("test_key".to_string()),
    };

    assert_eq!(config.rpc_port, 9545);
    assert_eq!(config.etherscan_api_key, Some("test_key".to_string()));
}

#[test]
fn test_config_clone() {
    let config = EngineConfig {
        rpc_port: 8080,
        etherscan_api_key: Some("key".to_string()),
    };

    let cloned = config.clone();

    assert_eq!(config.rpc_port, cloned.rpc_port);
    assert_eq!(config.etherscan_api_key, cloned.etherscan_api_key);
}
