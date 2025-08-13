//! RPC request handling and caching logic

use crate::cache::CacheManager;
use crate::providers::ProviderManager;
use eyre::Result;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn};

/// RPC methods that should be cached
const CACHEABLE_METHODS: &[&str] = &[
    "eth_getCode",
    "eth_getStorageAt",
    "eth_getTransactionByHash",
    "eth_getRawTransactionByHash",
    "eth_getTransactionReceipt",
    "eth_getBlockByNumber",
    "eth_getBlockByHash",
    "eth_getLogs",
    "eth_getProof",
    "eth_getBlockReceipts",
];

/// RPC request handler with intelligent caching capabilities
///
/// Handles incoming RPC requests by either serving from cache or forwarding to
/// upstream RPC servers. Implements smart caching logic that avoids caching
/// non-deterministic requests (e.g., "latest" block parameters).
pub struct RpcHandler {
    upstream_client: reqwest::Client,
    provider_manager: Arc<ProviderManager>,
    cache_manager: Arc<CacheManager>,
}

impl RpcHandler {
    /// Creates a new RPC handler with multiple providers and cache manager
    ///
    /// # Arguments
    /// * `provider_manager` - Manager for multiple RPC providers with load balancing
    /// * `cache_manager` - Shared cache manager for storing responses
    ///
    /// # Returns
    /// A new RpcHandler instance ready to process requests
    pub fn new(
        provider_manager: Arc<ProviderManager>,
        cache_manager: Arc<CacheManager>,
    ) -> Result<Self> {
        let upstream_client =
            reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build()?;

        Ok(Self { upstream_client, provider_manager, cache_manager })
    }

    /// Creates a new RPC handler with a single provider (backward compatibility)
    pub fn new_single(rpc_url: String, cache_manager: Arc<CacheManager>) -> Result<Self> {
        let provider_manager = Arc::new(tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { ProviderManager::new(vec![rpc_url], 3).await })
        })?);

        Self::new(provider_manager, cache_manager)
    }

    /// Returns a reference to the cache manager
    ///
    /// # Returns
    /// Reference to the underlying cache manager
    pub fn cache_manager(&self) -> &Arc<CacheManager> {
        &self.cache_manager
    }

    /// Returns a reference to the provider manager
    pub fn provider_manager(&self) -> &Arc<ProviderManager> {
        &self.provider_manager
    }

    /// Handles an RPC request with intelligent caching
    ///
    /// Determines whether to serve from cache or forward to upstream based on:
    /// - Whether the method is cacheable (see CACHEABLE_METHODS)
    /// - Whether the request contains non-deterministic block parameters
    /// - Whether a cached response already exists
    ///
    /// # Arguments
    /// * `request` - The JSON-RPC request to handle
    ///
    /// # Returns
    /// The JSON-RPC response, either from cache or upstream
    pub async fn handle_request(&self, request: Value) -> Result<Value> {
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        debug!("Handling RPC request: {}", method);

        // Check if this method should be cached
        if CACHEABLE_METHODS.contains(&method) {
            // Check if request has non-deterministic block parameters
            if self.has_non_deterministic_block_params(&request) {
                debug!("Non-deterministic block params for {}, bypassing cache", method);
                return self.forward_request(&request).await;
            }

            // Generate cache key from request
            let cache_key = self.generate_cache_key(&request);

            // Try to get from cache first
            if let Some(cached_response) = self.cache_manager.get(&cache_key).await {
                debug!("Cache hit for {}: {}", method, cache_key);
                return Ok(cached_response);
            }

            debug!("Cache miss for {}: {}", method, cache_key);

            // Forward to upstream and cache the result
            let response = self.forward_request(&request).await?;

            // Only cache successful responses
            if response.get("error").is_none() {
                self.cache_manager.set(cache_key, response.clone()).await;
            }

            Ok(response)
        } else {
            // Non-cacheable request - forward directly
            debug!("Non-cacheable request: {}", method);
            self.forward_request(&request).await
        }
    }

    async fn forward_request(&self, request: &Value) -> Result<Value> {
        const MAX_RETRIES: usize = 3;
        let mut last_error = None;

        for retry in 0..MAX_RETRIES {
            // Get next available provider
            let provider_url = match self.provider_manager.get_working_provider(3).await {
                Some(url) => url,
                None => {
                    warn!("No healthy providers available");
                    if retry < MAX_RETRIES - 1 {
                        // Trigger health check and wait before retry
                        self.provider_manager.health_check_all().await;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                    return Err(eyre::eyre!("No healthy RPC providers available"));
                }
            };

            debug!("Forwarding request to provider: {} (attempt {})", provider_url, retry + 1);
            let start = Instant::now();

            match self
                .upstream_client
                .post(&provider_url)
                .header("Content-Type", "application/json")
                .json(request)
                .send()
                .await
            {
                Ok(response) => {
                    let response_time = start.elapsed().as_millis() as u64;

                    match response.text().await {
                        Ok(response_text) => {
                            match serde_json::from_str::<Value>(&response_text) {
                                Ok(response_json) => {
                                    // Mark provider as successful
                                    self.provider_manager
                                        .mark_provider_success(&provider_url, response_time)
                                        .await;
                                    return Ok(response_json);
                                }
                                Err(e) => {
                                    warn!("Invalid JSON response from {}: {}", provider_url, e);
                                    last_error = Some(e.into());
                                    self.provider_manager.mark_provider_failed(&provider_url).await;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read response from {}: {}", provider_url, e);
                            last_error = Some(e.into());
                            self.provider_manager.mark_provider_failed(&provider_url).await;
                        }
                    }
                }
                Err(e) => {
                    warn!("Request failed to {}: {}", provider_url, e);
                    last_error = Some(e.into());
                    self.provider_manager.mark_provider_failed(&provider_url).await;
                }
            }

            // Wait before retry
            if retry < MAX_RETRIES - 1 {
                tokio::time::sleep(std::time::Duration::from_millis(100 * (retry as u64 + 1)))
                    .await;
            }
        }

        Err(last_error.unwrap_or_else(|| eyre::eyre!("All retry attempts failed")))
    }

    fn has_non_deterministic_block_params(&self, request: &Value) -> bool {
        let params = request.get("params").and_then(|p| p.as_array());

        if let Some(params) = params {
            for param in params {
                if let Some(param_str) = param.as_str() {
                    match param_str {
                        "latest" | "pending" | "earliest" | "safe" | "finalized" => {
                            return true;
                        }
                        _ => {}
                    }
                }
                // Also check object parameters for block identifiers
                if let Some(param_obj) = param.as_object() {
                    if let Some(block_value) = param_obj
                        .get("blockNumber")
                        .or_else(|| param_obj.get("toBlock"))
                        .or_else(|| param_obj.get("fromBlock"))
                    {
                        if let Some(block_str) = block_value.as_str() {
                            match block_str {
                                "latest" | "pending" | "earliest" | "safe" | "finalized" => {
                                    return true;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        false
    }

    fn generate_cache_key(&self, request: &Value) -> String {
        // Create a deterministic cache key from method + params
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").unwrap_or(&Value::Null);

        // For simplicity, we'll use a hash of the method + params
        // In production, you might want more sophisticated key generation
        format!("{}:{}", method, serde_json::to_string(params).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheManager;
    use tempfile::TempDir;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn create_test_rpc_handler() -> (RpcHandler, MockServer, TempDir) {
        let mock_server = MockServer::start().await;
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");

        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());
        let handler = RpcHandler::new_single(mock_server.uri(), cache_manager).unwrap();

        (handler, mock_server, temp_dir)
    }

    #[tokio::test]
    async fn test_cacheable_method_caching() {
        let (handler, mock_server, _temp_dir) = create_test_rpc_handler().await;

        let response_data = serde_json::json!({
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

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": ["0x1000000", false],
            "id": 1
        });

        // First request - should hit upstream
        let result1 = handler.handle_request(request.clone()).await.unwrap();
        assert_eq!(result1, response_data);

        // Second request - should hit cache
        let result2 = handler.handle_request(request).await.unwrap();
        assert_eq!(result2, response_data);

        // Mock server expectations should be met (only 1 call)
    }

    #[tokio::test]
    async fn test_non_cacheable_method_passthrough() {
        let (handler, mock_server, _temp_dir) = create_test_rpc_handler().await;

        let response_data = serde_json::json!({
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

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        // Both requests should hit upstream
        let result1 = handler.handle_request(request.clone()).await.unwrap();
        assert_eq!(result1, response_data);

        let result2 = handler.handle_request(request).await.unwrap();
        assert_eq!(result2, response_data);
    }

    #[tokio::test]
    async fn test_non_deterministic_block_params_bypass_cache() {
        let (handler, mock_server, _temp_dir) = create_test_rpc_handler().await;

        let response_data = serde_json::json!({
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
            .expect(2) // Should be called twice since "latest" bypasses cache
            .mount(&mock_server)
            .await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": ["latest", false],
            "id": 1
        });

        // Both requests should hit upstream due to "latest" parameter
        let result1 = handler.handle_request(request.clone()).await.unwrap();
        assert_eq!(result1, response_data);

        let result2 = handler.handle_request(request).await.unwrap();
        assert_eq!(result2, response_data);
    }

    #[tokio::test]
    async fn test_deterministic_vs_non_deterministic_params() {
        let (handler, mock_server, _temp_dir) = create_test_rpc_handler().await;

        let response_data = serde_json::json!({
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
            .expect(3) // Latest call twice + one specific block call
            .mount(&mock_server)
            .await;

        // Non-deterministic request with "latest"
        let latest_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": ["latest", false],
            "id": 1
        });

        // Deterministic request with specific block number
        let specific_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": ["0x1000000", false],
            "id": 1
        });

        // Latest requests should both hit upstream
        handler.handle_request(latest_request.clone()).await.unwrap();
        handler.handle_request(latest_request).await.unwrap();

        // Specific block request should hit upstream once
        handler.handle_request(specific_request.clone()).await.unwrap();
        // Second specific block request should hit cache (no additional upstream call)
        handler.handle_request(specific_request).await.unwrap();
    }

    #[test]
    fn test_has_non_deterministic_block_params() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test.json");
        let cache_manager = Arc::new(CacheManager::new(10, cache_path).unwrap());
        let handler =
            RpcHandler::new_single("http://example.com".to_string(), cache_manager).unwrap();

        // Test various non-deterministic block parameters
        let test_cases = vec![
            (
                serde_json::json!({
                    "method": "eth_getBlockByNumber",
                    "params": ["latest", false]
                }),
                true,
            ),
            (
                serde_json::json!({
                    "method": "eth_getBlockByNumber",
                    "params": ["pending", false]
                }),
                true,
            ),
            (
                serde_json::json!({
                    "method": "eth_getBlockByNumber",
                    "params": ["0x1000000", false]
                }),
                false,
            ),
            (
                serde_json::json!({
                    "method": "eth_getLogs",
                    "params": [{
                        "fromBlock": "latest",
                        "toBlock": "latest"
                    }]
                }),
                true,
            ),
            (
                serde_json::json!({
                    "method": "eth_getLogs",
                    "params": [{
                        "fromBlock": "0x1000000",
                        "toBlock": "0x1000010"
                    }]
                }),
                false,
            ),
        ];

        for (request, expected) in test_cases {
            let result = handler.has_non_deterministic_block_params(&request);
            assert_eq!(result, expected, "Failed for request: {:?}", request);
        }
    }

    #[tokio::test]
    async fn test_error_response_not_cached() {
        let (handler, mock_server, _temp_dir) = create_test_rpc_handler().await;

        let error_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32602,
                "message": "Invalid params"
            }
        });

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&error_response))
            .expect(2) // Should be called twice since errors aren't cached
            .mount(&mock_server)
            .await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": ["0x1000000", false],
            "id": 1
        });

        // Both requests should hit upstream since error responses aren't cached
        let result1 = handler.handle_request(request.clone()).await.unwrap();
        assert_eq!(result1, error_response);

        let result2 = handler.handle_request(request).await.unwrap();
        assert_eq!(result2, error_response);
    }
}
