use edb_tui::TuiConfig;
use std::time::Duration;

#[test]
fn test_default_tui_config() {
    let config = TuiConfig::default();

    assert_eq!(config.rpc_url, "http://localhost:8545");
    assert_eq!(config.refresh_interval, Duration::from_millis(100));
}

#[test]
fn test_custom_tui_config() {
    let config = TuiConfig {
        rpc_url: "http://localhost:9545".to_string(),
        refresh_interval: Duration::from_millis(500),
    };

    assert_eq!(config.rpc_url, "http://localhost:9545");
    assert_eq!(config.refresh_interval, Duration::from_millis(500));
}

#[test]
fn test_tui_config_clone() {
    let config = TuiConfig {
        rpc_url: "http://test:8545".to_string(),
        refresh_interval: Duration::from_secs(1),
    };

    let cloned = config.clone();

    assert_eq!(config.rpc_url, cloned.rpc_url);
    assert_eq!(config.refresh_interval, cloned.refresh_interval);
}
