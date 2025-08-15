//! Multi-provider RPC management with health checking and load balancing

use eyre::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Default Ethereum mainnet RPC endpoints
/// These are free public endpoints from chainlist.org, sorted by latency
pub const DEFAULT_MAINNET_RPCS: &[&str] = &[
    "https://rpc.eth.gateway.fm",
    "https://ethereum-rpc.publicnode.com",
    "https://mainnet.gateway.tenderly.co",
    "https://rpc.flashbots.net/fast",
    "https://rpc.flashbots.net",
    "https://gateway.tenderly.co/public/mainnet",
    "https://eth-mainnet.public.blastapi.io",
    "https://ethereum-mainnet.gateway.tatum.io",
    "https://eth.api.onfinality.io/public",
    "https://eth.llamarpc.com",
    "https://api.zan.top/eth-mainnet",
    "https://eth.drpc.org",
    "https://ethereum.rpc.subquery.network/public",
];

/// Information about an RPC provider
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// The RPC endpoint URL
    pub url: String,
    /// Whether the provider is currently healthy
    pub is_healthy: bool,
    /// When the provider was last health checked
    pub last_health_check: Option<Instant>,
    /// Response time in milliseconds for the last successful request
    pub response_time_ms: Option<u64>,
    /// Number of consecutive failures (reset on success)
    pub consecutive_failures: u32,
}

/// Serializable version of ProviderInfo for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfoResponse {
    /// The RPC endpoint URL
    pub url: String,
    /// Whether the provider is currently healthy
    pub is_healthy: bool,
    /// Seconds since the last health check
    pub last_health_check_seconds_ago: Option<u64>,
    /// Response time in milliseconds for the last successful request
    pub response_time_ms: Option<u64>,
    /// Number of consecutive failures (reset on success)
    pub consecutive_failures: u32,
}

impl From<&ProviderInfo> for ProviderInfoResponse {
    fn from(info: &ProviderInfo) -> Self {
        Self {
            url: info.url.clone(),
            is_healthy: info.is_healthy,
            last_health_check_seconds_ago: info.last_health_check.map(|t| t.elapsed().as_secs()),
            response_time_ms: info.response_time_ms,
            consecutive_failures: info.consecutive_failures,
        }
    }
}

/// Multi-provider manager with health checking and round-robin load balancing
pub struct ProviderManager {
    /// List of all providers (healthy and unhealthy)
    providers: Arc<RwLock<Vec<ProviderInfo>>>,
    /// Round-robin counter for load balancing
    round_robin_counter: AtomicUsize,
    /// HTTP client for health checks
    client: reqwest::Client,
    /// Maximum consecutive failures before marking unhealthy
    max_failures: u32,
}

impl ProviderManager {
    /// Create a new provider manager with the given RPC URLs
    pub async fn new(rpc_urls: Vec<String>, max_failures: u32) -> Result<Self> {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;

        let mut providers = Vec::new();

        // Initialize providers and perform initial health check
        for url in rpc_urls {
            let mut provider = ProviderInfo {
                url: url.clone(),
                is_healthy: false,
                last_health_check: None,
                response_time_ms: None,
                consecutive_failures: 0,
            };

            // Perform initial health check
            if let Ok(response_time) = Self::check_provider_health(&client, &url).await {
                provider.is_healthy = true;
                provider.response_time_ms = Some(response_time);
                provider.last_health_check = Some(Instant::now());
                info!("Provider {} is healthy ({}ms)", url, response_time);
            } else {
                warn!("Provider {} is not responding during initialization", url);
                provider.consecutive_failures = 1;
            }

            providers.push(provider);
        }

        // Ensure at least one provider is healthy
        let healthy_count = providers.iter().filter(|p| p.is_healthy).count();
        if healthy_count == 0 {
            return Err(eyre::eyre!("No healthy RPC providers available"));
        }

        info!("Initialized with {} healthy providers out of {}", healthy_count, providers.len());

        Ok(Self {
            providers: Arc::new(RwLock::new(providers)),
            round_robin_counter: AtomicUsize::new(0),
            client,
            max_failures,
        })
    }

    /// Check the health of a specific provider
    async fn check_provider_health(client: &reqwest::Client, url: &str) -> Result<u64> {
        let start = Instant::now();

        // Simple eth_blockNumber request to check if provider is responsive
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let response_time = start.elapsed().as_millis() as u64;

        // Check if we got a valid response
        let json: serde_json::Value = response.json().await?;
        if json.get("result").is_some() {
            Ok(response_time)
        } else {
            Err(eyre::eyre!("Invalid response from provider"))
        }
    }

    /// Get the next healthy provider using round-robin
    pub async fn get_next_provider(&self) -> Option<String> {
        let providers = self.providers.read().await;
        let healthy_providers: Vec<_> = providers.iter().filter(|p| p.is_healthy).collect();

        if healthy_providers.is_empty() {
            return None;
        }

        // Round-robin selection
        let index =
            self.round_robin_counter.fetch_add(1, Ordering::Relaxed) % healthy_providers.len();
        Some(healthy_providers[index].url.clone())
    }

    /// Get the current healthy provider using round-robin
    pub async fn get_current_provider(&self) -> Option<String> {
        let providers = self.providers.read().await;
        let healthy_providers: Vec<_> = providers.iter().filter(|p| p.is_healthy).collect();

        if healthy_providers.is_empty() {
            return None;
        }

        // Round-robin selection
        let index =
            self.round_robin_counter.load(Ordering::Relaxed) % healthy_providers.len();
        Some(healthy_providers[index].url.clone())
    }

    /// Mark a provider as failed and update its health status
    pub async fn mark_provider_failed(&self, url: &str) {
        let mut providers = self.providers.write().await;

        if let Some(provider) = providers.iter_mut().find(|p| p.url == url) {
            provider.consecutive_failures += 1;

            if provider.consecutive_failures >= self.max_failures {
                provider.is_healthy = false;
                debug!("Provider {} marked as unhealthy after {} failures", url, self.max_failures);
            }
        }
    }

    /// Mark a provider as successful and reset failure count
    pub async fn mark_provider_success(&self, url: &str, response_time_ms: u64) {
        let mut providers = self.providers.write().await;

        if let Some(provider) = providers.iter_mut().find(|p| p.url == url) {
            provider.consecutive_failures = 0;
            provider.is_healthy = true;
            provider.response_time_ms = Some(response_time_ms);
            provider.last_health_check = Some(Instant::now());

            debug!("Provider {} successful ({}ms)", url, response_time_ms);
        }
    }

    /// Perform health checks on all providers
    pub async fn health_check_all(&self) {
        let providers_snapshot = {
            let providers = self.providers.read().await;
            providers.clone()
        };

        for provider in providers_snapshot {
            // Check if provider needs health check (unhealthy or stale)
            let needs_check = !provider.is_healthy
                || provider
                    .last_health_check
                    .map_or(true, |t| t.elapsed() > Duration::from_secs(60));

            if needs_check {
                match Self::check_provider_health(&self.client, &provider.url).await {
                    Ok(response_time) => {
                        self.mark_provider_success(&provider.url, response_time).await;
                        if !provider.is_healthy {
                            info!("Provider {} is now healthy", provider.url);
                        }
                    }
                    Err(e) => {
                        debug!("Health check failed for {}: {}", provider.url, e);
                        self.mark_provider_failed(&provider.url).await;
                    }
                }
            }
        }
    }

    /// Get information about all providers (serializable version)
    pub async fn get_providers_info(&self) -> Vec<ProviderInfoResponse> {
        let providers = self.providers.read().await;
        providers.iter().map(|p| p.into()).collect()
    }

    /// Get count of healthy providers
    pub async fn healthy_provider_count(&self) -> usize {
        let providers = self.providers.read().await;
        providers.iter().filter(|p| p.is_healthy).count()
    }

    /// Try to get a working provider, with retries
    pub async fn get_working_provider(&self, max_retries: usize) -> Option<String> {
        for _ in 0..max_retries {
            if let Some(provider) = self.get_next_provider().await {
                return Some(provider);
            }

            // If no healthy providers, try health checking
            self.health_check_all().await;

            // Wait a bit before retry
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{debug, info};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_provider_initialization() {
        edb_utils::logging::ensure_test_logging(None);
        info!("Testing provider initialization with health checks");

        // Start mock servers
        let mock1 = MockServer::start().await;
        let mock2 = MockServer::start().await;

        // Setup successful response for mock1
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1234567"
            })))
            .mount(&mock1)
            .await;

        // Setup failed response for mock2
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock2)
            .await;

        let urls = vec![mock1.uri(), mock2.uri()];
        let manager = ProviderManager::new(urls, 3).await.unwrap();

        // Check that only mock1 is healthy
        assert_eq!(manager.healthy_provider_count().await, 1);

        let providers = manager.get_providers_info().await;
        assert_eq!(providers.len(), 2);
        assert!(providers[0].is_healthy);
        assert!(!providers[1].is_healthy);
    }

    #[tokio::test]
    async fn test_round_robin_selection() {
        edb_utils::logging::ensure_test_logging(None);
        info!("Testing round-robin provider selection");

        // Start 3 healthy mock servers
        let mocks =
            vec![MockServer::start().await, MockServer::start().await, MockServer::start().await];

        for mock in &mocks {
            Mock::given(method("POST"))
                .and(path("/"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": "0x1234567"
                })))
                .mount(mock)
                .await;
        }

        let urls: Vec<String> = mocks.iter().map(|m| m.uri()).collect();
        let manager = ProviderManager::new(urls.clone(), 3).await.unwrap();

        // Get providers multiple times and verify round-robin
        let mut selections = Vec::new();
        for _ in 0..9 {
            selections.push(manager.get_next_provider().await.unwrap());
        }

        // Each provider should be selected 3 times
        for url in &urls {
            assert_eq!(selections.iter().filter(|s| *s == url).count(), 3);
        }
    }

    #[tokio::test]
    async fn test_provider_failure_handling() {
        edb_utils::logging::ensure_test_logging(None);
        debug!("Testing provider failure detection and handling");

        let mock = MockServer::start().await;

        // Initially healthy
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1234567"
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let manager = ProviderManager::new(vec![mock.uri()], 2).await.unwrap();
        assert_eq!(manager.healthy_provider_count().await, 1);

        // Mark as failed twice (max_failures = 2)
        manager.mark_provider_failed(&mock.uri()).await;
        assert_eq!(manager.healthy_provider_count().await, 1); // Still healthy after 1 failure

        manager.mark_provider_failed(&mock.uri()).await;
        assert_eq!(manager.healthy_provider_count().await, 0); // Unhealthy after 2 failures
    }
}
