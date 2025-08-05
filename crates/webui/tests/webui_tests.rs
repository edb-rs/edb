use edb_webui::WebUiConfig;

#[test]
fn test_default_webui_config() {
    let config = WebUiConfig::default();

    assert_eq!(config.port, 3000);
    assert_eq!(config.engine_rpc_url, "http://localhost:8545");
}

#[test]
fn test_custom_webui_config() {
    let config = WebUiConfig { port: 8080, engine_rpc_url: "http://localhost:9545".to_string() };

    assert_eq!(config.port, 8080);
    assert_eq!(config.engine_rpc_url, "http://localhost:9545");
}

#[test]
fn test_webui_config_clone() {
    let config = WebUiConfig { port: 4000, engine_rpc_url: "http://test:8545".to_string() };

    let cloned = config.clone();

    assert_eq!(config.port, cloned.port);
    assert_eq!(config.engine_rpc_url, cloned.engine_rpc_url);
}
