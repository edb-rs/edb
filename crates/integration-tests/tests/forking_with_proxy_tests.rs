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

//! Integration tests for forking functionality with RPC proxy caching
//!
//! These tests demonstrate how the RPC proxy can be used to cache
//! blockchain data for more reliable and faster forking tests.

use alloy_primitives::TxHash;
use edb_common::fork_and_prepare;
use edb_rpc_proxy::proxy::ProxyServerBuilder;
use std::{path::PathBuf, time::Duration};
use tokio::time::sleep;
use tracing::{debug, info};

/// Test transaction hash from a known mainnet transaction
const TEST_TX_HASH: &str = "0xc403cced1cf53cbeb72475be7271b731f846e91fcbd7b43f120b8bbd60d5473e";

/// Another test transaction for testing multiple transactions in a block
const TEST_TX_HASH_2: &str = "0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060";

/// Helper to set up a test proxy with caching in testdata directory
async fn setup_test_proxy_with_cache(
    grace_period: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    // Get the testdata cache directory
    let cache_dir = get_testdata_cache_dir();

    let proxy = ProxyServerBuilder::new()
        .max_cache_items(20000)
        .cache_dir(cache_dir.clone())
        .grace_period(grace_period)
        .heartbeat_interval(30)
        .build()
        .await?;
    info!("Starting test proxy with cache at {}", cache_dir.display());

    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    drop(listener);

    // Start proxy in background
    tokio::spawn(async move {
        if let Err(e) = proxy.serve(addr).await {
            eprintln!("Proxy error: {}", e);
        }
    });

    // Wait for proxy to start
    sleep(Duration::from_millis(200)).await;

    let proxy_url = format!("http://{}", addr);
    println!("Test proxy started at {} with cache at {}", proxy_url, cache_dir.display());

    Ok(proxy_url)
}

/// Gracefully shutdown a test proxy using the edb_shutdown endpoint
async fn shutdown_test_proxy(proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let shutdown_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "edb_shutdown",
        "id": 1
    });

    match client.post(proxy_url).json(&shutdown_request).send().await {
        Ok(response) => {
            if response.status().is_success() {
                println!("Test proxy shutdown successfully");
                // Give the server a moment to shutdown gracefully
                sleep(Duration::from_millis(100)).await;
            } else {
                println!("Proxy shutdown request failed with status: {}", response.status());
            }
        }
        Err(e) => {
            // Connection refused is expected after shutdown
            if e.is_connect() {
                println!("Test proxy shutdown confirmed (connection refused)");
            } else {
                println!("Error during proxy shutdown: {}", e);
            }
        }
    }
    Ok(())
}

/// Get the testdata cache directory path
fn get_testdata_cache_dir() -> PathBuf {
    // Navigate from crates/integration-tests to project root
    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    // Look for workspace root (contains workspace Cargo.toml)
    let mut dir = current_dir.as_path();
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return dir.join("testdata").join("cache").join("rpc");
                }
            }
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    // Fallback
    current_dir.join("testdata").join("cache").join("rpc")
}

/// Register test instance with proxy for monitoring
async fn register_with_proxy(proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let register_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "edb_register",
        "params": [std::process::id(), chrono::Utc::now().timestamp()],
        "id": 1
    });

    client.post(proxy_url).json(&register_request).send().await?;
    println!("Registered test instance with proxy");
    Ok(())
}

/// Get cache statistics from proxy
async fn get_cache_stats(proxy_url: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let stats_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "edb_cache_stats",
        "id": 1
    });

    let response = client.post(proxy_url).json(&stats_request).send().await?;
    let body: serde_json::Value = response.json().await?;
    Ok(body["result"].clone())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_fork_with_proxy_cache() {
    edb_common::logging::ensure_test_logging(None);
    info!("Testing fork and prepare with proxy caching");

    let proxy_url = setup_test_proxy_with_cache(3000).await.expect("Failed to setup test proxy");

    register_with_proxy(&proxy_url).await.ok();

    let tx_hash: TxHash = TEST_TX_HASH.parse().expect("valid tx hash");

    println!("Testing fork with proxy caching...");
    let start = std::time::Instant::now();

    let result = fork_and_prepare(&proxy_url, tx_hash, false).await;
    let duration = start.elapsed();

    assert!(result.is_ok(), "Fork failed: {:?}", result.err());

    println!("First fork took: {:?}", duration);

    if let Ok(fork_result) = result {
        assert_eq!(fork_result.fork_info.chain_id, 1);
        assert_eq!(fork_result.fork_info.block_number, 23087459);

        // Test second fork (should be faster due to caching)
        let start2 = std::time::Instant::now();
        let result2 = fork_and_prepare(&proxy_url, tx_hash, false).await;
        let duration2 = start2.elapsed();

        assert!(result2.is_ok(), "Fork failed: {:?}", result2.err());

        println!("Second fork took: {:?}", duration2);

        // Print cache statistics
        if let Ok(stats) = get_cache_stats(&proxy_url).await {
            println!("Cache stats: {}", serde_json::to_string_pretty(&stats).unwrap_or_default());
        }

        println!("Fork with proxy cache test passed!");
    }

    // Gracefully shutdown the test proxy to ensure cache is saved
    shutdown_test_proxy(&proxy_url).await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_multiple_transactions_with_cache() {
    edb_common::logging::ensure_test_logging(None);
    info!("Testing multiple transactions with proxy caching");

    let proxy_url = setup_test_proxy_with_cache(30).await.expect("Failed to setup test proxy");

    register_with_proxy(&proxy_url).await.ok();

    let tx_hashes = [TEST_TX_HASH, TEST_TX_HASH_2];

    println!("Testing multiple transactions with caching...");

    for (i, tx_hash_str) in tx_hashes.iter().enumerate() {
        let tx_hash: TxHash = tx_hash_str.parse().expect("valid tx hash");

        println!("Testing transaction {}: {}", i + 1, tx_hash_str);
        let start = std::time::Instant::now();

        let result = fork_and_prepare(&proxy_url, tx_hash, false).await;
        let duration = start.elapsed();

        match result {
            Ok(fork_result) => {
                println!(
                    "Block: {}, Chain: {}, Time: {:?}",
                    fork_result.fork_info.block_number, fork_result.fork_info.chain_id, duration
                );
            }
            Err(e) => {
                panic!("Fork failed: {:?}", e);
            }
        }
    }

    // Print final cache statistics
    if let Ok(stats) = get_cache_stats(&proxy_url).await {
        println!("Final cache stats: {}", serde_json::to_string_pretty(&stats).unwrap_or_default());
    }

    // Gracefully shutdown the test proxy to ensure cache is saved
    shutdown_test_proxy(&proxy_url).await.ok();
}

#[tokio::test]
async fn test_proxy_endpoints() {
    edb_common::logging::ensure_test_logging(None);
    debug!("Testing proxy endpoint functionality");

    let proxy_url = setup_test_proxy_with_cache(30).await.expect("Failed to setup test proxy");

    let client = reqwest::Client::new();

    // Test ping endpoint
    let ping_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "edb_ping",
        "id": 1
    });

    let response =
        client.post(&proxy_url).json(&ping_request).send().await.expect("Failed to ping proxy");
    assert!(response.status().is_success());

    // Test info endpoint
    let info_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "edb_info",
        "id": 1
    });

    let response =
        client.post(&proxy_url).json(&info_request).send().await.expect("Failed to get proxy info");
    assert!(response.status().is_success());

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["result"]["service"], "edb-rpc-proxy");

    // Test cache stats
    let stats = get_cache_stats(&proxy_url).await.expect("Failed to get cache stats");
    assert!(stats["max_entries"].as_u64().unwrap() > 0);

    println!("All proxy endpoints working correctly!");

    // Gracefully shutdown the test proxy to ensure cache is saved
    shutdown_test_proxy(&proxy_url).await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_fork_and_prepare_quick_mode() {
    edb_common::logging::ensure_test_logging(None);
    info!("Testing fork and prepare in quick mode with caching");

    // Setup proxy with cache for more reliable testing
    let proxy_url = setup_test_proxy_with_cache(30).await.expect("Failed to setup test proxy");

    register_with_proxy(&proxy_url).await.ok();

    // Use a transaction that's not the first in its block
    let tx_hash: TxHash = TEST_TX_HASH.parse().expect("valid tx hash");

    println!("Testing quick mode fork with cached proxy...");

    // Test with quick mode enabled (should be faster)
    let start_quick = std::time::Instant::now();
    let result_quick = fork_and_prepare(&proxy_url, tx_hash, true).await;
    let duration_quick = start_quick.elapsed();

    match result_quick {
        Ok(fork_result) => {
            // Should still get correct fork info
            assert_eq!(fork_result.fork_info.block_number, 23087459);
            assert_eq!(fork_result.fork_info.spec_id, revm::primitives::hardfork::SpecId::CANCUN);

            println!(
                "Quick mode fork completed in {:?} at block {}",
                duration_quick, fork_result.fork_info.block_number
            );
        }
        Err(e) => {
            panic!("Failed to fork in quick mode: {e:?}");
        }
    }

    // Compare with normal mode (should have same fork info but different state)
    let start_normal = std::time::Instant::now();
    let result_normal = fork_and_prepare(&proxy_url, tx_hash, false).await;
    let duration_normal = start_normal.elapsed();

    match result_normal {
        Ok(fork_result) => {
            assert_eq!(fork_result.fork_info.block_number, 23087459);
            assert_eq!(fork_result.fork_info.spec_id, revm::primitives::hardfork::SpecId::CANCUN);

            println!(
                "Normal mode fork completed in {:?} at block {}",
                duration_normal, fork_result.fork_info.block_number
            );

            // Quick mode should typically be faster
            println!(
                "Quick mode was {}x faster",
                duration_normal.as_secs_f64() / duration_quick.as_secs_f64()
            );
        }
        Err(e) => {
            panic!("Failed to fork in normal mode: {e:?}");
        }
    }

    // Print cache statistics to see cache utilization
    if let Ok(stats) = get_cache_stats(&proxy_url).await {
        println!(
            "Cache stats after quick mode test: {}",
            serde_json::to_string_pretty(&stats).unwrap_or_default()
        );
    }

    // Gracefully shutdown the test proxy to ensure cache is saved
    shutdown_test_proxy(&proxy_url).await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_fork_at_specific_hardfork_boundaries() {
    edb_common::logging::ensure_test_logging(None);
    info!("Testing fork at different hardfork boundaries");

    // Setup proxy with cache for more reliable testing
    let proxy_url = setup_test_proxy_with_cache(30).await.expect("Failed to setup test proxy");

    register_with_proxy(&proxy_url).await.ok();

    // Test transactions at different hardfork boundaries
    struct HardforkTest {
        tx_hash: &'static str,
        expected_block: u64,
        expected_spec: revm::primitives::hardfork::SpecId,
        description: &'static str,
    }

    let tests = vec![
        HardforkTest {
            // Transaction from Frontier era
            tx_hash: TEST_TX_HASH_2,
            expected_block: 46147,
            expected_spec: revm::primitives::hardfork::SpecId::FRONTIER,
            description: "Frontier era transaction",
        },
        // Add more test cases for different eras as needed
        // Note: You can add more hardfork boundary tests here as needed
    ];

    println!("Testing fork at different hardfork boundaries with cached proxy...");

    for test in tests {
        println!("Testing: {}", test.description);
        let tx_hash: TxHash = test.tx_hash.parse().expect("valid tx hash");

        let start = std::time::Instant::now();
        match fork_and_prepare(&proxy_url, tx_hash, false).await {
            Ok(fork_result) => {
                let duration = start.elapsed();

                assert_eq!(
                    fork_result.fork_info.block_number, test.expected_block,
                    "{}: Wrong block number",
                    test.description
                );
                assert_eq!(
                    fork_result.fork_info.spec_id, test.expected_spec,
                    "{}: Wrong spec ID",
                    test.description
                );

                println!(
                    "{} passed - Block: {}, Spec: {:?}, Time: {:?}",
                    test.description,
                    fork_result.fork_info.block_number,
                    fork_result.fork_info.spec_id,
                    duration
                );
            }
            Err(e) => {
                panic!("{} failed: {:?}", test.description, e);
            }
        }
    }

    // Print final cache statistics
    if let Ok(stats) = get_cache_stats(&proxy_url).await {
        println!(
            "Cache stats after hardfork boundary tests: {}",
            serde_json::to_string_pretty(&stats).unwrap_or_default()
        );
    }

    // Gracefully shutdown the test proxy to ensure cache is saved
    shutdown_test_proxy(&proxy_url).await.ok();
}
