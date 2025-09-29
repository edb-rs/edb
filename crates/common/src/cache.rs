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

//! Cache utilities.

use std::{fs, marker::PhantomData, path::PathBuf, time::Duration};

use alloy_chains::Chain;
use eyre::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::{trace, warn};

/// Default cache TTL for etherscan.
/// Set to 1 day since the source code of a contract is unlikely to change frequently.
pub const DEFAULT_ETHERSCAN_CACHE_TTL: u64 = 86400;

/// Trait for cache paths.
pub trait CachePath {
    /// Returns the path to edb's cache dir: `~/.edb/cache` by default.
    fn edb_cache_dir(&self) -> Option<PathBuf>;

    /// Check whether the cache is valid.
    fn is_valid(&self) -> bool {
        self.edb_cache_dir().is_some()
    }

    /// Returns the path to edb rpc cache dir: `<cache_root>/rpc`.
    fn edb_rpc_cache_dir(&self) -> Option<PathBuf> {
        Some(self.edb_cache_dir()?.join("rpc"))
    }
    /// Returns the path to edb chain's cache dir: `<cache_root>/rpc/<chain>`
    fn rpc_chain_cache_dir(&self, chain_id: impl Into<Chain>) -> Option<PathBuf> {
        Some(self.edb_rpc_cache_dir()?.join(chain_id.into().to_string()))
    }

    /// Returns the path to edb's etherscan cache dir: `<cache_root>/etherscan`.
    fn etherscan_cache_dir(&self) -> Option<PathBuf> {
        Some(self.edb_cache_dir()?.join("etherscan"))
    }

    /// Returns the path to edb's etherscan cache dir for `chain_id`:
    /// `<cache_root>/etherscan/<chain>`
    fn etherscan_chain_cache_dir(&self, chain_id: impl Into<Chain>) -> Option<PathBuf> {
        Some(self.etherscan_cache_dir()?.join(chain_id.into().to_string()))
    }

    /// Returns the path to edb's compiler cache dir: `<cache_root>/solc`.
    fn compiler_cache_dir(&self) -> Option<PathBuf> {
        Some(self.edb_cache_dir()?.join("solc"))
    }

    /// Returns the path to edb's compiler cache dir for `chain_id`:
    /// `<cache_root>/solc/<chain>`
    fn compiler_chain_cache_dir(&self, chain_id: impl Into<Chain>) -> Option<PathBuf> {
        Some(self.compiler_cache_dir()?.join(chain_id.into().to_string()))
    }
}

/// Cache path for edb.
#[derive(Debug)]
pub struct EdbCachePath {
    root: Option<PathBuf>,
}

impl Default for EdbCachePath {
    fn default() -> Self {
        Self { root: dirs_next::home_dir().map(|p| p.join(".edb").join("cache")) }
    }
}

impl EdbCachePath {
    /// New cache path.
    pub fn new(root: Option<impl Into<PathBuf>>) -> Self {
        Self {
            root: root
                .map(Into::into)
                .or_else(|| dirs_next::home_dir().map(|p| p.join(".edb").join("cache"))),
        }
    }

    /// New empty cache path.
    pub fn empty() -> Self {
        Self { root: None }
    }
}

impl CachePath for EdbCachePath {
    fn edb_cache_dir(&self) -> Option<PathBuf> {
        self.root.clone()
    }
}

impl CachePath for Option<EdbCachePath> {
    fn edb_cache_dir(&self) -> Option<PathBuf> {
        self.as_ref()?.edb_cache_dir()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheWrapper<T> {
    pub data: T,
    pub expires_at: u64,
}

impl<T> CacheWrapper<T> {
    pub fn new(data: T, ttl: Option<Duration>) -> Self {
        Self {
            data,
            expires_at: ttl
                .map(|ttl| ttl.as_secs().saturating_add(chrono::Utc::now().timestamp() as u64))
                .unwrap_or(u64::MAX),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at < chrono::Utc::now().timestamp() as u64
    }
}

/// Trait for cache.
pub trait Cache {
    /// The type of the data to be cached.
    type Data: Serialize + DeserializeOwned;

    /// Loads the cache for the given label.
    fn load_cache(&self, label: impl Into<String>) -> Option<Self::Data>;

    /// Saves the cache for the given label.
    fn save_cache(&self, label: impl Into<String>, data: &Self::Data) -> Result<()>;
}

/// A cache manager that stores data in the file system.
///  - `T` is the type of the data to be cached.
///  - `cache_dir` is the directory where the cache files are stored.
///  - `cache_ttl` is the time-to-live of the cache files. If it is `None`, the cache files will
///    never expire.
#[derive(Debug, Clone)]
pub struct EdbCache<T> {
    cache_dir: PathBuf,
    cache_ttl: Option<Duration>,
    phantom: PhantomData<T>,
}

impl<T> EdbCache<T>
where
    T: Serialize + DeserializeOwned,
{
    /// New cache.
    pub fn new(
        cache_dir: Option<impl Into<PathBuf>>,
        cache_ttl: Option<Duration>,
    ) -> Result<Option<Self>> {
        if let Some(cache_dir) = cache_dir {
            let cache_dir = cache_dir.into();
            fs::create_dir_all(&cache_dir)?;
            Ok(Some(Self { cache_dir, cache_ttl, phantom: PhantomData }))
        } else {
            Ok(None)
        }
    }

    /// Returns the cache directory.
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Returns the cache TTL.
    pub fn cache_ttl(&self) -> Option<Duration> {
        self.cache_ttl
    }
}

impl<T> Cache for EdbCache<T>
where
    T: Serialize + DeserializeOwned,
{
    type Data = T;

    fn load_cache(&self, label: impl Into<String>) -> Option<T> {
        let cache_file = self.cache_dir.join(format!("{}.json", label.into()));
        trace!("loading cache: {:?}", cache_file);
        if !cache_file.exists() {
            return None;
        }

        let content = fs::read_to_string(&cache_file).ok()?;
        let cache: CacheWrapper<_> = if let Ok(cache) = serde_json::from_str(&content) {
            cache
        } else {
            warn!("the cache file has been corrupted: {:?}", cache_file);
            let _ = fs::remove_file(&cache_file); // we do not care about the result
            return None;
        };

        if cache.is_expired() {
            trace!("the cache file has expired: {:?}", cache_file);
            let _ = fs::remove_file(&cache_file); // we do not care about the result
            None
        } else {
            trace!("hit the cache: {:?}", cache_file);
            Some(cache.data)
        }
    }

    fn save_cache(&self, label: impl Into<String>, data: &T) -> Result<()> {
        let cache_file = self.cache_dir.join(format!("{}.json", label.into()));
        trace!("saving cache: {:?}", cache_file);

        let cache = CacheWrapper::new(data, self.cache_ttl);
        let content = serde_json::to_string(&cache)?;
        fs::write(&cache_file, content)?;
        Ok(())
    }
}

impl<T> Cache for Option<EdbCache<T>>
where
    T: Serialize + DeserializeOwned,
{
    type Data = T;

    fn load_cache(&self, label: impl Into<String>) -> Option<T> {
        self.as_ref()?.load_cache(label)
    }

    fn save_cache(&self, label: impl Into<String>, data: &T) -> Result<()> {
        if let Some(cache) = self {
            cache.save_cache(label, data)
        } else {
            Ok(())
        }
    }
}
