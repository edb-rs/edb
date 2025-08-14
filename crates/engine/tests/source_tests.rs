use edb_engine::source::SourceApiKeys;
use tracing::{debug, info, warn};

#[test]
fn test_source_api_keys_default() {
    edb_utils::logging::ensure_test_logging();
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
    edb_utils::logging::ensure_test_logging();
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
    edb_utils::logging::ensure_test_logging();
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
