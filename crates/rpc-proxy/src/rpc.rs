//! RPC request handling and caching logic

use crate::cache::CacheManager;
use crate::providers::ProviderManager;
use eyre::Result;
use reqwest::StatusCode;
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

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
    "eth_getBalance",
];

/// Common rate limit error patterns found in various RPC providers
const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate-limit",
    "ratelimit",
    "too many requests",
    "cu limit exceeded",
    "compute units exceeded",
    "quota exceeded",
    "throttled",
    "exceeded the allowed rps",
    "request limit",
    "max requests",
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
                debug!(
                    "Cache hit for {}: {} ({})",
                    method,
                    cached_response.to_string().chars().take(200).collect::<String>(),
                    cache_key
                );
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

    /// Detects if a response indicates rate limiting
    ///
    /// Checks HTTP status codes, response text patterns, and JSON error messages
    /// to identify rate limiting from various providers
    fn is_rate_limit_response(
        &self,
        status: StatusCode,
        response_text: &str,
        json_response: Option<&Value>,
    ) -> bool {
        // Check HTTP status code
        if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
            debug!("Rate limit response detected by status: {}", status);
            return true;
        }

        // Check response text for rate limit patterns
        let text_lower = response_text.to_lowercase();
        if RATE_LIMIT_PATTERNS.iter().any(|pattern| text_lower.contains(pattern)) {
            debug!(
                "Rate limit response detected by response text: {}",
                &text_lower.chars().take(200).collect::<String>()
            );
            return true;
        }

        // Check JSON error response
        if let Some(json) = json_response {
            if let Some(error) = json.get("error") {
                // Check error message
                if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
                    let msg_lower = message.to_lowercase();
                    if RATE_LIMIT_PATTERNS.iter().any(|pattern| msg_lower.contains(pattern)) {
                        debug!(
                            "Rate limit response detected by error message: {}",
                            &msg_lower.chars().take(200).collect::<String>()
                        );
                        return true;
                    }
                }

                // Check error code (some providers use specific codes for rate limiting)
                if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
                    match code {
                        429 | -32005 | -32098 | -32099 => {
                            debug!("Rate limit response detected by error code: {}", code);
                            return true;
                        }
                        _ => {}
                    }
                }
            }
        }

        false
    }

    /// Detects if an error is due to user's invalid request
    ///
    /// These errors should be returned immediately without trying other providers
    fn is_user_error(&self, json_response: &Value) -> bool {
        if let Some(error) = json_response.get("error") {
            if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
                match code {
                    -32600 | -32601 | -32602 | -32700 => {
                        debug!("Detected user error with code: {}", code);
                        return true;
                    }
                    _ => {}
                }
            }

            // Also check for specific error messages that indicate user error
            if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
                let msg_lower = message.to_lowercase();

                // Common user error patterns
                const USER_ERROR_PATTERNS: &[&str] = &[
                    "invalid method",
                    "method not found",
                    "invalid params",
                    "missing param",
                    "invalid argument",
                    "parse error",
                    "invalid request",
                    "unsupported method",
                ];

                if USER_ERROR_PATTERNS.iter().any(|pattern| msg_lower.contains(pattern)) {
                    debug!("Detected user error from message: {}", message);
                    return true;
                }
            }
        }

        false
    }

    /// Creates a hash-based signature for error deduplication
    ///
    /// Uses a stable hash of the error object to identify similar errors
    fn create_error_signature(&self, error: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash the error code if present
        if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
            code.hash(&mut hasher);
        }

        // Hash the error message if present (normalized to lowercase)
        if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
            message.to_lowercase().hash(&mut hasher);
        }

        // Optionally hash error data if it's a simple type
        if let Some(data) = error.get("data") {
            match data {
                Value::String(s) => s.hash(&mut hasher),
                Value::Number(n) => n.to_string().hash(&mut hasher),
                Value::Bool(b) => b.hash(&mut hasher),
                _ => {} // Skip complex data structures
            }
        }

        hasher.finish()
    }

    async fn forward_request(&self, request: &Value) -> Result<Value> {
        const MAX_RETRIES: usize = 3;
        const MAX_MULTIPLE_SAME_ERROR: usize = 3;

        // Track errors from different providers using hash as key
        let mut error_responses: HashMap<u64, (Value, usize)> = HashMap::new();
        let mut last_network_error: Option<eyre::Error> = None;
        let mut providers_tried = 0;

        for retry in 0..MAX_RETRIES {
            // Get next available provider
            let provider_url = match self.provider_manager.get_working_provider(3).await {
                Some(url) => url,
                None => {
                    warn!("No healthy providers available (attempt {})", retry + 1);
                    if retry < MAX_RETRIES - 1 {
                        // Trigger health check and wait before retry
                        self.provider_manager.health_check_all().await;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }

                    // If we have any error responses, return the most common one
                    if let Some((error_response, _)) =
                        error_responses.values().max_by_key(|(_, count)| *count)
                    {
                        info!("All providers exhausted, returning most common error");
                        return Ok(error_response.clone());
                    }

                    return Err(last_network_error
                        .unwrap_or_else(|| eyre::eyre!("No healthy RPC providers available")));
                }
            };

            providers_tried += 1;
            debug!(
                "Forwarding request to provider: {} (attempt {}/{})",
                provider_url,
                retry + 1,
                MAX_RETRIES
            );

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
                    let status = response.status();
                    let response_time = start.elapsed().as_millis() as u64;

                    match response.text().await {
                        Ok(response_text) => {
                            // Try to parse as JSON
                            let json_result = serde_json::from_str::<Value>(&response_text);

                            // Check if it's a rate limit error
                            if self.is_rate_limit_response(
                                status,
                                &response_text,
                                json_result.as_ref().ok(),
                            ) {
                                warn!(
                                    "Provider {} is rate limited (response: {}...)",
                                    provider_url,
                                    &response_text.chars().take(200).collect::<String>()
                                );

                                self.provider_manager.mark_provider_failed(&provider_url).await;

                                // Continue to next provider without counting as error
                                continue;
                            }

                            // Handle valid JSON response
                            match json_result {
                                Ok(response_json) => {
                                    // Check if response contains an error
                                    if let Some(error) = response_json.get("error") {
                                        // Check if it's a user error (invalid request)
                                        if self.is_user_error(&response_json) {
                                            info!("Detected user error, returning immediately");
                                            // Don't mark provider as failed - it's user's fault
                                            self.provider_manager
                                                .mark_provider_success(&provider_url, response_time)
                                                .await;
                                            return Ok(response_json);
                                        }

                                        // It's a provider/blockchain error
                                        let error_hash = self.create_error_signature(error);

                                        // Track this error
                                        error_responses
                                            .entry(error_hash)
                                            .and_modify(|(_, count)| *count += 1)
                                            .or_insert((response_json.clone(), 1));

                                        debug!(
                                            "Provider {} returned error (hash: {})",
                                            provider_url, error_hash
                                        );

                                        // If multiple providers return the same error, it's likely legitimate
                                        if let Some((_, count)) = error_responses.get(&error_hash) {
                                            if *count >= MAX_MULTIPLE_SAME_ERROR {
                                                info!(
                                                    "Multiple providers ({}) returned same error, likely legitimate",
                                                    count
                                                );
                                                return Ok(response_json);
                                            }
                                        }

                                        // Mark provider as failed and continue
                                        self.provider_manager
                                            .mark_provider_failed(&provider_url)
                                            .await;
                                        continue;
                                    }

                                    // Success! No error in response
                                    self.provider_manager
                                        .mark_provider_success(&provider_url, response_time)
                                        .await;

                                    debug!(
                                        "Request successful via {} ({}ms)",
                                        provider_url, response_time
                                    );
                                    return Ok(response_json);
                                }
                                Err(parse_error) => {
                                    // Response is not valid JSON
                                    warn!(
                                        "Invalid JSON response from {} (first 200 chars): {}...",
                                        provider_url,
                                        &response_text.chars().take(200).collect::<String>()
                                    );

                                    self.provider_manager.mark_provider_failed(&provider_url).await;
                                    last_network_error = Some(eyre::eyre!(
                                        "Invalid JSON from provider: {}",
                                        parse_error
                                    ));
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read response body from {}: {}", provider_url, e);
                            self.provider_manager.mark_provider_failed(&provider_url).await;
                            last_network_error = Some(e.into());
                            continue;
                        }
                    }
                }
                Err(e) => {
                    warn!("Request failed to {}: {}", provider_url, e);
                    self.provider_manager.mark_provider_failed(&provider_url).await;
                    last_network_error = Some(e.into());
                    continue;
                }
            }
        }

        // All retries exhausted
        info!("All {} retries exhausted after trying {} providers", MAX_RETRIES, providers_tried);

        // Return the most common error if we have any
        if !error_responses.is_empty() {
            let (most_common_error, count) =
                error_responses.values().max_by_key(|(_, count)| *count).unwrap();

            info!("Returning most common error (seen {} times)", count);
            return Ok(most_common_error.clone());
        }

        // Otherwise return the last network error
        Err(last_network_error.unwrap_or_else(|| eyre::eyre!("429 Too Many Requests")))
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

        // Set up health check response that ProviderManager needs during initialization
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"  // eth_blockNumber/eth_chainid response for health check
            })))
            .up_to_n_times(1) // Only for the initial health check
            .mount(&mock_server)
            .await;

        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        // Now create provider manager with mock server URL
        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock_server.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

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

    #[tokio::test]
    async fn test_has_non_deterministic_block_params() {
        let (handler, _mock_server, _temp_dir) = create_test_rpc_handler().await;

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

    #[tokio::test]
    async fn test_multi_provider_round_robin() {
        // Create 3 mock servers for round-robin testing
        let mock1 = MockServer::start().await;
        let mock2 = MockServer::start().await;
        let mock3 = MockServer::start().await;

        // Setup health check responses for all providers
        for mock_server in &[&mock1, &mock2, &mock3] {
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": "0x1234567"
                })))
                .up_to_n_times(1) // Health check
                .mount(mock_server)
                .await;
        }

        // Create cache and provider manager with all 3 mock servers
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock1.uri(), mock2.uri(), mock3.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

        // Setup different responses for each mock to verify round-robin
        let response1 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "response_from_server_1"
        });

        let response2 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "response_from_server_2"
        });

        let response3 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "response_from_server_3"
        });

        // Setup expectations for non-cacheable method (eth_blockNumber) to test round-robin
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response1))
            .expect(3) // Each server should get exactly 3 requests
            .mount(&mock1)
            .await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response2))
            .expect(3)
            .mount(&mock2)
            .await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response3))
            .expect(3)
            .mount(&mock3)
            .await;

        // Send 9 requests for non-cacheable method to verify round-robin distribution
        let mut responses = Vec::new();
        for i in 0..9 {
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber", // Non-cacheable method
                "params": [],
                "id": i + 1
            });

            let response = handler.handle_request(request).await.unwrap();
            responses.push(response["result"].as_str().unwrap().to_string());
        }

        // Verify that each server response appears exactly 3 times (round-robin)
        let server1_count = responses.iter().filter(|r| *r == "response_from_server_1").count();
        let server2_count = responses.iter().filter(|r| *r == "response_from_server_2").count();
        let server3_count = responses.iter().filter(|r| *r == "response_from_server_3").count();

        assert_eq!(server1_count, 3, "Server 1 should receive exactly 3 requests");
        assert_eq!(server2_count, 3, "Server 2 should receive exactly 3 requests");
        assert_eq!(server3_count, 3, "Server 3 should receive exactly 3 requests");

        // Verify round-robin order: should cycle through servers 1,2,3,1,2,3,1,2,3
        let expected_pattern = vec![
            "response_from_server_1",
            "response_from_server_2",
            "response_from_server_3",
            "response_from_server_1",
            "response_from_server_2",
            "response_from_server_3",
            "response_from_server_1",
            "response_from_server_2",
            "response_from_server_3",
        ];

        assert_eq!(responses, expected_pattern, "Requests should follow round-robin pattern");
    }

    #[tokio::test]
    async fn test_multi_provider_caching_behavior() {
        // Create 2 mock servers
        let mock1 = MockServer::start().await;
        let mock2 = MockServer::start().await;

        // Setup health check responses
        for mock_server in &[&mock1, &mock2] {
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": "0x1234567"
                })))
                .up_to_n_times(1)
                .mount(mock_server)
                .await;
        }

        // Create provider manager
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock1.uri(), mock2.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

        let cacheable_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "number": "0x1000000",
                "hash": "0x1234567890abcdef"
            }
        });

        // Setup expectations: Only ONE server should be hit for cacheable requests
        // Due to round-robin, the first request will go to mock1
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&cacheable_response))
            .expect(1) // Should only be called once due to caching
            .mount(&mock1)
            .await;

        // mock2 should not be called at all for the cached request
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&cacheable_response))
            .expect(0) // Should not be called
            .mount(&mock2)
            .await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber", // Cacheable method
            "params": ["0x1000000", false],
            "id": 1
        });

        // Send the same cacheable request 3 times
        for _ in 0..3 {
            let response = handler.handle_request(request.clone()).await.unwrap();
            assert_eq!(response, cacheable_response);
        }

        // The mock expectations will verify that only mock1 was called once
    }

    #[tokio::test]
    async fn test_rate_limit_detection() {
        // Create test handler with minimal setup
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        // Mock server for testing (needed for provider manager init)
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock_server.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

        // Test various rate limit responses
        let test_cases = vec![
            // HTTP 429 status
            (StatusCode::TOO_MANY_REQUESTS, "{}", None, true),
            // HTTP 503 status
            (StatusCode::SERVICE_UNAVAILABLE, "{}", None, true),
            // Text pattern "rate limit"
            (StatusCode::OK, "rate limit exceeded", None, true),
            // Text pattern "cu limit exceeded" (Zan RPC)
            (StatusCode::OK, "cu limit exceeded; The current RPC traffic is too high", None, true),
            // JSON error with rate limit message
            (
                StatusCode::OK,
                "{}",
                Some(serde_json::json!({
                    "error": {
                        "code": -32005,
                        "message": "Too many requests"
                    }
                })),
                true,
            ),
            // JSON error with rate limit code
            (
                StatusCode::OK,
                "{}",
                Some(serde_json::json!({
                    "error": {
                        "code": 429,
                        "message": "Some error"
                    }
                })),
                true,
            ),
            // Normal response (not rate limited)
            (
                StatusCode::OK,
                "{}",
                Some(serde_json::json!({
                    "result": "0x1234"
                })),
                false,
            ),
            // Normal error (not rate limited)
            (
                StatusCode::OK,
                "{}",
                Some(serde_json::json!({
                    "error": {
                        "code": -32000,
                        "message": "Execution reverted"
                    }
                })),
                false,
            ),
        ];

        for (status, text, json, expected) in test_cases {
            let result = handler.is_rate_limit_response(status, text, json.as_ref());
            assert_eq!(
                result, expected,
                "Failed for status: {:?}, text: {}, json: {:?}",
                status, text, json
            );
        }
    }

    #[tokio::test]
    async fn test_user_error_detection() {
        // Similar minimal setup
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock_server.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

        // Test various user error patterns
        let test_cases = vec![
            // Invalid Request
            (
                serde_json::json!({
                    "error": {
                        "code": -32600,
                        "message": "Invalid Request"
                    }
                }),
                true,
            ),
            // Method not found
            (
                serde_json::json!({
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                }),
                true,
            ),
            // Invalid params
            (
                serde_json::json!({
                    "error": {
                        "code": -32602,
                        "message": "Invalid params"
                    }
                }),
                true,
            ),
            // Parse error
            (
                serde_json::json!({
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    }
                }),
                true,
            ),
            // Message-based detection
            (
                serde_json::json!({
                    "error": {
                        "code": -32000,
                        "message": "Invalid method eth_fooBar"
                    }
                }),
                true,
            ),
            // Blockchain execution error (not user error)
            (
                serde_json::json!({
                    "error": {
                        "code": -32000,
                        "message": "Execution reverted"
                    }
                }),
                false,
            ),
            // Rate limit error (not user error)
            (
                serde_json::json!({
                    "error": {
                        "code": 429,
                        "message": "Too many requests"
                    }
                }),
                false,
            ),
        ];

        for (response, expected) in test_cases {
            let result = handler.is_user_error(&response);
            assert_eq!(result, expected, "Failed for response: {:?}", response);
        }
    }

    #[tokio::test]
    async fn test_rate_limit_fallback_to_healthy_provider() {
        // Create 2 mock servers - first returns rate limit, second succeeds
        let mock1 = MockServer::start().await;
        let mock2 = MockServer::start().await;

        // Setup health check responses
        for mock_server in &[&mock1, &mock2] {
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": "0x1"
                })))
                .up_to_n_times(1)
                .mount(mock_server)
                .await;
        }

        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock1.uri(), mock2.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

        // First provider returns rate limit error
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429).set_body_string("Rate limit exceeded"))
            .expect(1)
            .mount(&mock1)
            .await;

        // Second provider returns success
        let success_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "0x12345"
        });

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&success_response))
            .expect(1)
            .mount(&mock2)
            .await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        // Should get success from second provider despite first being rate limited
        let result = handler.handle_request(request).await.unwrap();
        assert_eq!(result, success_response);
    }

    #[tokio::test]
    async fn test_error_deduplication() {
        // Create 3 mock servers that all return the same error
        let mock1 = MockServer::start().await;
        let mock2 = MockServer::start().await;
        let mock3 = MockServer::start().await;

        // Setup health checks
        for mock_server in &[&mock1, &mock2, &mock3] {
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": "0x1"
                })))
                .up_to_n_times(1)
                .mount(mock_server)
                .await;
        }

        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");
        let cache_manager = Arc::new(CacheManager::new(100, cache_path).unwrap());

        let provider_manager = Arc::new(
            ProviderManager::new(vec![mock1.uri(), mock2.uri(), mock3.uri()], 3)
                .await
                .expect("Failed to create provider manager"),
        );

        let handler = RpcHandler::new(provider_manager, cache_manager).unwrap();

        // All providers return the same blockchain error
        let error_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32000,
                "message": "Execution reverted: insufficient balance"
            }
        });

        // First provider
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&error_response))
            .expect(1)
            .mount(&mock1)
            .await;

        // Second provider - same error should trigger early return
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&error_response))
            .expect(1)
            .mount(&mock2)
            .await;

        // Third provider should NOT be called (2 matching errors is enough)
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&error_response))
            .expect(0)
            .mount(&mock3)
            .await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{}, "latest"],
            "id": 1
        });

        // Should return the error after 2 providers agree
        let result = handler.handle_request(request).await.unwrap();
        assert_eq!(result, error_response);
    }
}
