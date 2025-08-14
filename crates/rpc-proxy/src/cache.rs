//! In-memory cache manager for RPC responses with disk persistence

use edb_utils::{
    cache::{CachePath, EDBCachePath},
    forking,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// A cached RPC response entry with metadata
///
/// This struct holds a cached RPC response along with its creation timestamp
/// for LRU eviction purposes.
#[derive(Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The cached RPC response data
    pub data: Value,
    /// Unix timestamp when this entry was created
    pub created_at: u64,
}

impl CacheEntry {
    fn new(data: Value) -> Self {
        Self {
            data,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// In-memory cache manager for RPC responses with disk persistence
///
/// Manages a thread-safe in-memory cache with LRU eviction and provides
/// functionality to persist the cache to disk as JSON.
pub struct CacheManager {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_items: u32,
    cache_file_path: PathBuf,
}

impl CacheManager {
    /// Creates a new cache manager with the specified capacity and disk path
    ///
    /// # Arguments
    /// * `max_items` - Maximum number of items to store in the cache
    /// * `cache_path` - Path where the cache will be persisted to disk
    ///
    /// # Returns
    /// A new CacheManager instance, loading any existing cache from disk
    pub fn new(max_items: u32, cache_path: PathBuf) -> Result<Self> {
        info!("Using cache file: {}", cache_path.display());

        // Load existing cache from disk
        let cache = if cache_path.exists() {
            match fs::read_to_string(&cache_path) {
                Ok(content) => {
                    match serde_json::from_str::<HashMap<String, CacheEntry>>(&content) {
                        Ok(loaded_cache) => {
                            info!("Loaded {} cache entries from disk", loaded_cache.len());
                            loaded_cache
                        }
                        Err(e) => {
                            warn!("Failed to parse cache file, starting with empty cache: {}", e);
                            HashMap::new()
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read cache file, starting with empty cache: {}", e);
                    HashMap::new()
                }
            }
        } else {
            info!("No existing cache file found, starting with empty cache");
            HashMap::new()
        };

        Ok(Self { cache: Arc::new(RwLock::new(cache)), max_items, cache_file_path: cache_path })
    }

    /// Retrieves a cached value by key
    ///
    /// # Arguments
    /// * `key` - The cache key to look up
    ///
    /// # Returns
    /// The cached value if found, None otherwise
    pub async fn get(&self, key: &str) -> Option<Value> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(key) {
            debug!("Cache hit: {}", key);
            Some(entry.data.clone())
        } else {
            debug!("Cache miss: {}", key);
            None
        }
    }

    /// Stores a value in the cache with the given key
    ///
    /// If the cache is at capacity, this will trigger LRU eviction of the oldest entries.
    ///
    /// # Arguments
    /// * `key` - The cache key to store under
    /// * `value` - The value to cache
    pub async fn set(&self, key: String, value: Value) {
        let mut cache = self.cache.write().await;

        // Check if we need to evict entries to make space
        if cache.len() >= self.max_items as usize {
            self.evict_oldest(&mut cache).await;
        }

        let entry = CacheEntry::new(value);
        cache.insert(key.clone(), entry);
        debug!("Cached entry: {}", key);
    }

    async fn evict_oldest(&self, cache: &mut HashMap<String, CacheEntry>) {
        // Sort entries by creation time (oldest first) and remove oldest 10%
        let to_remove = (cache.len() / 10).max(1);

        let mut entries: Vec<(String, u64)> =
            cache.iter().map(|(key, entry)| (key.clone(), entry.created_at)).collect();

        // Sort by created_at (oldest first)
        entries.sort_by_key(|(_, created_at)| *created_at);

        let keys_to_remove: Vec<String> =
            entries.into_iter().take(to_remove).map(|(key, _)| key).collect();

        for key in &keys_to_remove {
            cache.remove(key);
        }

        warn!("Evicted {} oldest cache entries due to size limit", keys_to_remove.len());
    }

    /// Saves the current cache contents to disk as JSON
    ///
    /// # Returns
    /// Result indicating success or failure of the save operation
    pub async fn save_to_disk(&self) -> Result<()> {
        let cache = self.cache.read().await;
        let content = serde_json::to_string_pretty(&*cache)?;
        fs::write(&self.cache_file_path, content)?;
        info!("Saved {} cache entries to disk", cache.len());
        Ok(())
    }

    /// Returns detailed statistics about the cache state
    ///
    /// # Returns
    /// A JSON object containing cache utilization, entry counts, and timing information
    pub async fn detailed_stats(&self) -> serde_json::Value {
        let cache = self.cache.read().await;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut oldest_entry = None;
        let mut newest_entry = None;

        for entry in cache.values() {
            if oldest_entry.is_none() || entry.created_at < oldest_entry.unwrap() {
                oldest_entry = Some(entry.created_at);
            }
            if newest_entry.is_none() || entry.created_at > newest_entry.unwrap() {
                newest_entry = Some(entry.created_at);
            }
        }

        serde_json::json!({
            "total_entries": cache.len(),
            "max_entries": self.max_items,
            "utilization": format!("{:.1}%", (cache.len() as f64 / self.max_items as f64) * 100.0),
            "oldest_entry_age_seconds": oldest_entry.map(|t| current_time.saturating_sub(t)),
            "newest_entry_age_seconds": newest_entry.map(|t| current_time.saturating_sub(t)),
            "cache_file_path": self.cache_file_path.display().to_string(),
        })
    }

    /// Returns all cache entries for testing purposes
    ///
    /// This method is primarily intended for testing and debugging.
    /// In production, prefer using specific cache queries or statistics.
    ///
    /// # Returns
    /// A HashMap containing all cache entries with their keys and values
    #[allow(dead_code)]
    pub async fn get_all_entries(&self) -> HashMap<String, CacheEntry> {
        let cache = self.cache.read().await;
        cache.clone()
    }

    /// Generates a cache file path based on the RPC URL and optional cache directory
    ///
    /// Creates a chain-specific cache directory structure and ensures parent directories exist.
    ///
    /// # Arguments
    /// * `rpc_url` - The RPC endpoint URL to determine chain ID from
    /// * `cache_dir` - Optional base cache directory (defaults to ~/.edb/cache)
    ///
    /// # Returns
    /// The full path to the cache file for this RPC endpoint
    pub async fn get_cache_path(
        rpc_urls: &Vec<String>,
        cache_dir: Option<PathBuf>,
    ) -> Result<PathBuf> {
        let chain_ids: HashSet<_> =
            futures::future::join_all(rpc_urls.iter().map(|url| forking::get_chain_id(url)))
                .await
                .into_iter()
                .filter_map(Result::ok)
                .collect();

        if chain_ids.len() != 1 {
            eyre::bail!("All RPC URLs must belong to the same chain. Found: {:?}", chain_ids);
        }

        let chain_id = *chain_ids.iter().next().unwrap();

        let cache_path = EDBCachePath::new(cache_dir)
            .rpc_chain_cache_dir(chain_id)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rpc.json");

        // Create directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(cache_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};
    use tracing::{debug, info};

    fn create_test_cache_manager(max_items: u32) -> (CacheManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_rpc.json");
        let manager = CacheManager::new(max_items, cache_path).unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_cache_get_set() {
        edb_utils::logging::ensure_test_logging();
        info!("Testing cache get/set operations");

        let (manager, _temp_dir) = create_test_cache_manager(10);

        // Test cache miss
        assert!(manager.get("test_key").await.is_none());

        // Test cache set and get
        let test_value = serde_json::json!({"result": "test_data"});
        manager.set("test_key".to_string(), test_value.clone()).await;

        let retrieved = manager.get("test_key").await.unwrap();
        assert_eq!(retrieved, test_value);
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        edb_utils::logging::ensure_test_logging();
        info!("Testing cache eviction behavior");

        let (manager, _temp_dir) = create_test_cache_manager(3);

        // Fill cache to capacity with delays to ensure different timestamps
        for i in 0..3 {
            let key = format!("key_{}", i);
            let value = serde_json::json!({"data": i});
            manager.set(key, value).await;
            sleep(Duration::from_millis(50)).await;
        }

        // Add one more item to trigger eviction
        manager.set("key_3".to_string(), serde_json::json!({"data": 3})).await;

        // Check that oldest item was evicted (key_0 should be gone)
        assert!(manager.get("key_0").await.is_none());
        assert!(manager.get("key_3").await.is_some());
    }

    #[tokio::test]
    async fn test_cache_eviction_order() {
        edb_utils::logging::ensure_test_logging();
        info!("Testing cache eviction order");

        let (manager, _temp_dir) = create_test_cache_manager(3);

        // Add items with delays to ensure different timestamps
        manager.set("old_key".to_string(), serde_json::json!({"data": "old"})).await;
        sleep(Duration::from_millis(50)).await;

        manager.set("mid_key".to_string(), serde_json::json!({"data": "mid"})).await;
        sleep(Duration::from_millis(50)).await;

        manager.set("new_key".to_string(), serde_json::json!({"data": "new"})).await;
        sleep(Duration::from_millis(50)).await;

        // Trigger eviction
        manager.set("newest_key".to_string(), serde_json::json!({"data": "newest"})).await;

        // The oldest item should be evicted
        assert!(manager.get("old_key").await.is_none());
        assert!(manager.get("mid_key").await.is_some());
        assert!(manager.get("new_key").await.is_some());
        assert!(manager.get("newest_key").await.is_some());
    }

    #[tokio::test]
    async fn test_cache_persistence() {
        edb_utils::logging::ensure_test_logging();
        info!("Testing cache persistence across restarts");

        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("persist_test.json");

        // Create first cache manager and add data
        {
            let manager = CacheManager::new(10, cache_path.clone()).unwrap();
            manager.set("persist_key".to_string(), serde_json::json!({"persisted": true})).await;
            manager.save_to_disk().await.unwrap();
        } // manager goes out of scope

        // Create new cache manager with same path
        let manager2 = CacheManager::new(10, cache_path).unwrap();
        let retrieved = manager2.get("persist_key").await.unwrap();
        assert_eq!(retrieved, serde_json::json!({"persisted": true}));
    }

    #[tokio::test]
    async fn test_detailed_stats() {
        edb_utils::logging::ensure_test_logging();
        info!("Testing detailed cache statistics");

        let (manager, _temp_dir) = create_test_cache_manager(100);

        // Test empty cache stats
        let stats = manager.detailed_stats().await;
        assert_eq!(stats["total_entries"], 0);
        assert_eq!(stats["max_entries"], 100);
        assert_eq!(stats["utilization"], "0.0%");
        assert!(stats["oldest_entry_age_seconds"].is_null());
        assert!(stats["newest_entry_age_seconds"].is_null());

        // Add some items
        manager.set("item1".to_string(), serde_json::json!({"data": 1})).await;
        sleep(Duration::from_millis(100)).await;
        manager.set("item2".to_string(), serde_json::json!({"data": 2})).await;

        // Test stats with items
        let stats = manager.detailed_stats().await;
        assert_eq!(stats["total_entries"], 2);
        assert_eq!(stats["max_entries"], 100);
        assert_eq!(stats["utilization"], "2.0%");
        assert!(
            stats["oldest_entry_age_seconds"].as_u64().unwrap()
                >= stats["newest_entry_age_seconds"].as_u64().unwrap()
        );
    }

    #[tokio::test]
    async fn test_cache_entry_timestamps() {
        edb_utils::logging::ensure_test_logging();
        debug!("Testing cache entry timestamp behavior");

        let entry1 = CacheEntry::new(serde_json::json!({"test": 1}));
        sleep(Duration::from_millis(10)).await;
        let entry2 = CacheEntry::new(serde_json::json!({"test": 2}));

        assert!(entry2.created_at >= entry1.created_at);
    }
}
