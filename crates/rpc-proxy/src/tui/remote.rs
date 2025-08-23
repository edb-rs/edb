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


//! Remote proxy client for TUI monitoring

use eyre::Result;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, warn};

/// Remote proxy client for communicating with proxy servers via RPC
#[derive(Clone)]
pub struct RemoteProxyClient {
    client: reqwest::Client,
    proxy_url: String,
}

impl RemoteProxyClient {
    /// Create a new remote proxy client
    pub fn new(proxy_url: String, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, proxy_url }
    }

    /// Make an RPC request to the proxy
    async fn rpc_request(&self, method: &str) -> Result<Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "id": 1
        });

        debug!("Making RPC request to {}: {}", self.proxy_url, method);

        let response = self.client.post(&self.proxy_url).json(&request).send().await?;

        if !response.status().is_success() {
            eyre::bail!("HTTP error: {}", response.status());
        }

        let body: Value = response.json().await?;

        if let Some(error) = body.get("error") {
            eyre::bail!("RPC error: {}", error);
        }

        Ok(body.get("result").unwrap_or(&Value::Null).clone())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<Value> {
        self.rpc_request("edb_cache_stats").await
    }

    /// Get enhanced cache metrics
    pub async fn get_cache_metrics(&self) -> Result<Value> {
        self.rpc_request("edb_cache_metrics").await
    }

    /// Get provider information
    pub async fn get_providers(&self) -> Result<Value> {
        self.rpc_request("edb_providers").await
    }

    /// Get provider metrics
    pub async fn get_provider_metrics(&self) -> Result<Value> {
        self.rpc_request("edb_provider_metrics").await
    }

    /// Get active instances
    pub async fn get_active_instances(&self) -> Result<Value> {
        self.rpc_request("edb_active_instances").await
    }

    /// Get metrics history
    pub async fn get_metrics_history(&self) -> Result<Value> {
        self.rpc_request("edb_metrics_history").await
    }

    /// Get real-time request metrics
    pub async fn get_request_metrics(&self) -> Result<Value> {
        self.rpc_request("edb_request_metrics").await
    }

    /// Check if proxy is reachable
    pub async fn ping(&self) -> Result<Value> {
        self.rpc_request("edb_ping").await
    }

    /// Get proxy info
    pub async fn get_info(&self) -> Result<Value> {
        self.rpc_request("edb_info").await
    }
}

/// Data structures for remote monitoring (converted from JSON responses)

#[derive(Debug, Clone)]
pub struct RemoteCacheStats {
    pub total_entries: u64,
    pub max_entries: u64,
    pub utilization: String,
    pub cache_file_path: String,
    pub oldest_entry_age_seconds: Option<u64>,
    pub newest_entry_age_seconds: Option<u64>,
}

impl RemoteCacheStats {
    pub fn from_json(value: &Value) -> Result<Self> {
        Ok(Self {
            total_entries: value.get("total_entries").and_then(|v| v.as_u64()).unwrap_or(0),
            max_entries: value.get("max_entries").and_then(|v| v.as_u64()).unwrap_or(0),
            utilization: value
                .get("utilization")
                .and_then(|v| v.as_str())
                .unwrap_or("0%")
                .to_string(),
            cache_file_path: value
                .get("cache_file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            oldest_entry_age_seconds: value
                .get("oldest_entry_age_seconds")
                .and_then(|v| v.as_u64()),
            newest_entry_age_seconds: value
                .get("newest_entry_age_seconds")
                .and_then(|v| v.as_u64()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RemoteProviderStatus {
    pub url: String,
    pub is_healthy: bool,
    pub consecutive_failures: u32,
    pub response_time_ms: Option<u64>,
    pub last_health_check_seconds_ago: Option<u64>,
}

impl RemoteProviderStatus {
    pub fn from_json(value: &Value) -> Result<Self> {
        Ok(Self {
            url: value.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            is_healthy: value.get("is_healthy").and_then(|v| v.as_bool()).unwrap_or(false),
            consecutive_failures: value
                .get("consecutive_failures")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            response_time_ms: value.get("response_time_ms").and_then(|v| v.as_u64()),
            last_health_check_seconds_ago: value
                .get("last_health_check_seconds_ago")
                .and_then(|v| v.as_u64()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RemoteMetricData {
    pub timestamp: u64,
    pub _cache_hits: u64,
    pub _cache_misses: u64,
    pub cache_size: u64,
    pub hit_rate: f64, // Store hit rate directly from backend
    pub _healthy_providers: u64,
    pub _total_providers: u64,
    pub requests_per_minute: u64,
    pub avg_response_time_ms: f64,
    pub _active_instances: usize,
}

impl RemoteMetricData {
    pub fn from_history_json(cache_history: &[Value], provider_history: &[Value]) -> Vec<Self> {
        let mut metrics = Vec::new();

        // Combine cache and provider history data
        for (cache_data, provider_data) in cache_history.iter().zip(provider_history.iter()) {
            // Extract cache hits and misses from the history data
            let cache_hits = cache_data.get("cache_hits").and_then(|v| v.as_u64()).unwrap_or(0);
            let cache_misses = cache_data.get("cache_misses").and_then(|v| v.as_u64()).unwrap_or(0);

            // Extract hit_rate directly from backend
            let hit_rate = cache_data.get("hit_rate").and_then(|v| v.as_f64()).unwrap_or(0.0);

            metrics.push(Self {
                timestamp: cache_data.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                _cache_hits: cache_hits,
                _cache_misses: cache_misses,
                cache_size: cache_data.get("cache_size").and_then(|v| v.as_u64()).unwrap_or(0),
                hit_rate,
                _healthy_providers: provider_data
                    .get("healthy_providers")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                _total_providers: provider_data
                    .get("total_providers")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                requests_per_minute: cache_data
                    .get("requests_per_minute")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                avg_response_time_ms: provider_data
                    .get("avg_response_time_ms")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                _active_instances: cache_data
                    .get("active_instances")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
            });
        }

        metrics
    }
}

/// Remote data fetcher that collects all necessary data for TUI
pub struct RemoteDataFetcher {
    client: RemoteProxyClient,
}

impl RemoteDataFetcher {
    pub fn new(client: RemoteProxyClient) -> Self {
        Self { client }
    }

    /// Fetch all data needed for TUI display
    pub async fn fetch_all_data(&self) -> Result<RemoteProxyData> {
        // Fetch data in parallel for better performance
        let (
            cache_stats_result,
            cache_metrics_result,
            providers_result,
            provider_metrics_result,
            instances_result,
            history_result,
            request_metrics_result,
            info_result,
        ) = tokio::join!(
            self.client.get_cache_stats(),
            self.client.get_cache_metrics(),
            self.client.get_providers(),
            self.client.get_provider_metrics(),
            self.client.get_active_instances(),
            self.client.get_metrics_history(),
            self.client.get_request_metrics(),
            self.client.get_info(),
        );

        // Convert results and handle errors gracefully
        let cache_stats = cache_stats_result
            .map_err(|e| warn!("Failed to fetch cache stats: {}", e))
            .ok()
            .and_then(|v| RemoteCacheStats::from_json(&v).ok());

        let providers_data =
            providers_result.map_err(|e| warn!("Failed to fetch providers: {}", e)).ok();

        let providers = providers_data
            .as_ref()
            .and_then(|v| v.get("providers"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|p| RemoteProviderStatus::from_json(p).ok()).collect())
            .unwrap_or_default();

        let active_instances = instances_result
            .map_err(|e| warn!("Failed to fetch instances: {}", e))
            .ok()
            .and_then(|v| {
                v.get("active_instances").and_then(|a| {
                    a.as_array().map(|arr| {
                        arr.iter().filter_map(|v| v.as_u64().map(|u| u as u32)).collect()
                    })
                })
            })
            .unwrap_or_default();

        let metrics_history = history_result
            .map_err(|e| warn!("Failed to fetch history: {}", e))
            .ok()
            .map(|v| {
                let empty_vec = Vec::new();
                let cache_history =
                    v.get("cache_history").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
                let provider_history =
                    v.get("provider_history").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
                RemoteMetricData::from_history_json(cache_history, provider_history)
            })
            .unwrap_or_default();

        Ok(RemoteProxyData {
            cache_stats,
            providers,
            active_instances,
            metrics_history,
            cache_metrics: cache_metrics_result.ok(),
            provider_metrics: provider_metrics_result.ok(),
            request_metrics: request_metrics_result.ok(),
            system_info: info_result.ok(),
        })
    }
}

/// Combined data structure for remote proxy monitoring
#[derive(Debug, Clone)]
pub struct RemoteProxyData {
    pub cache_stats: Option<RemoteCacheStats>,
    pub providers: Vec<RemoteProviderStatus>,
    pub active_instances: Vec<u32>,
    pub metrics_history: Vec<RemoteMetricData>,
    pub cache_metrics: Option<Value>,
    pub provider_metrics: Option<Value>,
    pub request_metrics: Option<Value>,
    pub system_info: Option<Value>,
}
