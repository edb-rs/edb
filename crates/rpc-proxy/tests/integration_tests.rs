//! Integration tests for the RPC proxy server

use edb_rpc_proxy::proxy::ProxyServer;
use reqwest::Client;
use serde_json::{json, Value};
use std::{net::SocketAddr, time::Duration};
use tempfile::TempDir;
use tokio::time::sleep;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

/// Helper to create a test proxy server
async fn create_test_proxy(upstream_url: String, max_cache_items: u32) -> (ProxyServer, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = Some(temp_dir.path().to_path_buf());

    let proxy = ProxyServer::new(
        upstream_url,
        max_cache_items,
        cache_dir,
        300, // grace_period: 5 minutes
        10,  // heartbeat_interval: 10 seconds
    )
    .await
    .unwrap();

    (proxy, temp_dir)
}

/// Start proxy server on a random port and return the address
async fn start_proxy_server(proxy: ProxyServer) -> SocketAddr {
    // Find an available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let actual_addr = listener.local_addr().unwrap();
    drop(listener); // Release the listener so proxy.serve can bind to it

    tokio::spawn(async move {
        proxy.serve(actual_addr).await.unwrap();
    });

    // Give the server a moment to start
    sleep(Duration::from_millis(200)).await;
    actual_addr
}

#[tokio::test]
async fn test_proxy_health_endpoints() {
    let (_proxy, _temp_dir) = create_test_proxy("http://example.com".to_string(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    // Test edb_ping
    let ping_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_ping",
        "id": 1
    });

    let response = client.post(&proxy_url).json(&ping_request).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["result"], "pong");

    // Test edb_info
    let info_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_info",
        "id": 2
    });

    let response = client.post(&proxy_url).json(&info_request).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["result"]["service"], "edb-rpc-proxy");
}

#[tokio::test]
async fn test_proxy_registry_endpoints() {
    let (_proxy, _temp_dir) = create_test_proxy("http://example.com".to_string(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    // Test edb_register
    let register_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_register",
        "params": [12345, 1234567890],
        "id": 1
    });

    let response = client.post(&proxy_url).json(&register_request).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["result"]["status"], "registered");
    assert_eq!(body["result"]["pid"], 12345);

    // Test edb_heartbeat
    let heartbeat_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_heartbeat",
        "params": [12345],
        "id": 2
    });

    let response = client.post(&proxy_url).json(&heartbeat_request).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["result"]["status"], "ok");
    assert_eq!(body["result"]["pid"], 12345);
}

#[tokio::test]
async fn test_proxy_cache_stats_endpoint() {
    let (_proxy, _temp_dir) = create_test_proxy("http://example.com".to_string(), 100).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    // Test edb_cache_stats
    let stats_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_cache_stats",
        "id": 1
    });

    let response = client.post(&proxy_url).json(&stats_request).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();

    // Verify stats structure
    let stats = &body["result"];
    assert!(stats["total_entries"].is_number());
    assert_eq!(stats["max_entries"], 100);
    assert!(stats["utilization"].is_string());
    assert!(stats["cache_file_path"].is_string());
}

#[tokio::test]
async fn test_proxy_active_instances_endpoint() {
    // Set up mock upstream server for the proxy
    let mock_server = MockServer::start().await;

    // Mock the eth_chainId call that happens during cache path setup
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "0x1"
        })))
        .mount(&mock_server)
        .await;

    let (_proxy, _temp_dir) = create_test_proxy(mock_server.uri(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    // First, register a couple of EDB instances
    let register_request1 = json!({
        "jsonrpc": "2.0",
        "method": "edb_register",
        "params": [12345, 1234567890],
        "id": 1
    });

    let register_request2 = json!({
        "jsonrpc": "2.0",
        "method": "edb_register",
        "params": [54321, 1234567891],
        "id": 2
    });

    // Register both instances
    client.post(&proxy_url).json(&register_request1).send().await.unwrap();
    client.post(&proxy_url).json(&register_request2).send().await.unwrap();

    // Now test the active instances endpoint
    let active_instances_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_active_instances",
        "id": 3
    });

    let response = client.post(&proxy_url).json(&active_instances_request).send().await.unwrap();

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();

    // Verify response structure
    let result = &body["result"];
    assert!(result["active_instances"].is_array());
    assert_eq!(result["count"], 2);

    let active_pids = result["active_instances"].as_array().unwrap();
    let pids: Vec<u64> = active_pids.iter().map(|v| v.as_u64().unwrap()).collect();

    // Should contain both registered PIDs
    assert!(pids.contains(&12345));
    assert!(pids.contains(&54321));
}

#[tokio::test]
async fn test_proxy_rpc_forwarding_and_caching() {
    // Set up mock upstream server
    let mock_server = MockServer::start().await;

    let response_data = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "number": "0x1000000",
            "hash": "0x1234567890abcdef"
        }
    });

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_data))
        .expect(1) // Should only be called once due to caching
        .mount(&mock_server)
        .await;

    // Create proxy pointing to mock server
    let (_proxy, _temp_dir) = create_test_proxy(mock_server.uri(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    let rpc_request = json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": ["0x1000000", false],
        "id": 1
    });

    // First request - should hit upstream
    let response1 = client.post(&proxy_url).json(&rpc_request).send().await.unwrap();

    assert_eq!(response1.status(), 200);
    let body1: Value = response1.json().await.unwrap();
    assert_eq!(body1, response_data);

    // Second request - should hit cache
    let response2 = client.post(&proxy_url).json(&rpc_request).send().await.unwrap();

    assert_eq!(response2.status(), 200);
    let body2: Value = response2.json().await.unwrap();
    assert_eq!(body2, response_data);

    // Mock server expectations should be met (only 1 call)
}

#[tokio::test]
async fn test_proxy_non_cacheable_forwarding() {
    // Set up mock upstream server
    let mock_server = MockServer::start().await;

    let response_data = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": "0x1000000"
    });

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_data))
        .expect(2) // Should be called twice since not cacheable
        .mount(&mock_server)
        .await;

    // Create proxy pointing to mock server
    let (_proxy, _temp_dir) = create_test_proxy(mock_server.uri(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    let rpc_request = json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber", // Non-cacheable method
        "params": [],
        "id": 1
    });

    // Both requests should hit upstream
    let response1 = client.post(&proxy_url).json(&rpc_request).send().await.unwrap();

    assert_eq!(response1.status(), 200);
    let body1: Value = response1.json().await.unwrap();
    assert_eq!(body1, response_data);

    let response2 = client.post(&proxy_url).json(&rpc_request).send().await.unwrap();

    assert_eq!(response2.status(), 200);
    let body2: Value = response2.json().await.unwrap();
    assert_eq!(body2, response_data);

    // Mock server expectations should be met (2 calls)
}

#[tokio::test]
async fn test_proxy_non_deterministic_params_bypass_cache() {
    // Set up mock upstream server
    let mock_server = MockServer::start().await;

    let response_data = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "number": "0x1000001", // Different block number to show it's not cached
            "hash": "0xabcdef1234567890"
        }
    });

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response_data))
        .expect(2) // Should be called twice since "latest" bypasses cache
        .mount(&mock_server)
        .await;

    // Create proxy pointing to mock server
    let (_proxy, _temp_dir) = create_test_proxy(mock_server.uri(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    let rpc_request = json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": ["latest", false], // "latest" should bypass cache
        "id": 1
    });

    // Both requests should hit upstream due to "latest" parameter
    let response1 = client.post(&proxy_url).json(&rpc_request).send().await.unwrap();

    assert_eq!(response1.status(), 200);
    let body1: Value = response1.json().await.unwrap();
    assert_eq!(body1, response_data);

    let response2 = client.post(&proxy_url).json(&rpc_request).send().await.unwrap();

    assert_eq!(response2.status(), 200);
    let body2: Value = response2.json().await.unwrap();
    assert_eq!(body2, response_data);

    // Mock server expectations should be met (2 calls)
}

#[tokio::test]
async fn test_proxy_invalid_request_handling() {
    let (_proxy, _temp_dir) = create_test_proxy("http://example.com".to_string(), 10).await;
    let proxy_addr = start_proxy_server(_proxy).await;

    let client = Client::new();
    let proxy_url = format!("http://{}", proxy_addr);

    // Test request without method
    let invalid_request = json!({
        "jsonrpc": "2.0",
        "id": 1
    });

    let response = client.post(&proxy_url).json(&invalid_request).send().await.unwrap();

    assert_eq!(response.status(), 400); // Bad Request

    // Test malformed edb_register request (missing params)
    let malformed_request = json!({
        "jsonrpc": "2.0",
        "method": "edb_register",
        "id": 1
    });

    let response = client.post(&proxy_url).json(&malformed_request).send().await.unwrap();

    assert_eq!(response.status(), 400); // Bad Request
}
