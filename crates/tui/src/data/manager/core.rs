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

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    mem,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use eyre::Result;
use tokio::sync::RwLock;
use tracing::debug;

use crate::RpcClient;

/// A cache map that tracks three states for each key:
/// 1. Key not in map = Not fetched
/// 2. Key maps to Some(V) = Fetched with valid value
/// 3. Key maps to None = Fetched but no valid value
#[derive(Debug, Clone)]
pub struct FetchCache<K, V> {
    data: HashMap<K, Option<V>>,
}

impl<K, V> Deref for FetchCache<K, V> {
    type Target = HashMap<K, Option<V>>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<K, V> DerefMut for FetchCache<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

// IntoIterator for owned FetchCache (moves out its contents)
impl<K, V> IntoIterator for FetchCache<K, V> {
    type Item = (K, Option<V>);
    type IntoIter = std::collections::hash_map::IntoIter<K, Option<V>>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

// IntoIterator for &FetchCache (shared iteration)
impl<'a, K, V> IntoIterator for &'a FetchCache<K, V> {
    type Item = (&'a K, &'a Option<V>);
    type IntoIter = std::collections::hash_map::Iter<'a, K, Option<V>>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

// IntoIterator for &mut FetchCache (mutable iteration)
impl<'a, K, V> IntoIterator for &'a mut FetchCache<K, V> {
    type Item = (&'a K, &'a mut Option<V>);
    type IntoIter = std::collections::hash_map::IterMut<'a, K, Option<V>>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}

impl<K, V> FetchCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// Check if the cache has an entry for the given key
    pub fn has_cached(&self, key: &K) -> bool {
        self.data.contains_key(key)
    }

    /// Check if reference has new entries that are not in cache
    pub fn need_update(&self, reference: &FetchCache<K, V>) -> bool {
        reference.keys().any(|key| !self.data.contains_key(key))
    }

    /// Update cache based on reference map
    /// - Adds/updates entries from reference as Some(value)
    /// - Marks keys not in reference as None (fetched-invalid)
    pub fn update(&mut self, reference: &FetchCache<K, V>) {
        // Add/update entries from reference
        for (key, value) in reference {
            if !self.data.contains_key(key) {
                self.data.insert(key.clone(), value.clone());
            }
        }
    }
}

impl<K, V> Default for FetchCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

pub trait ManagerStateTr: Sized {
    /// Create a new state with RPC client
    fn with_rpc_client(
        rpc_client: Arc<RpcClient>,
    ) -> impl std::future::Future<Output = Result<Self>> + Send;

    /// Update the state with another state's data
    fn update(&mut self, reference: &Self);
}

pub trait ManagerRequestTr<S> {
    fn fetch_data(
        self,
        rpc_client: Arc<RpcClient>,
        state: &mut S,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

#[derive(Debug, Clone)]
pub struct ManagerCore<S, R>
where
    S: ManagerStateTr,
    R: Eq + Hash + ManagerRequestTr<S>,
{
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,
    /// Cached state
    pub(super) state: S,
    /// Pending requests to be processed
    pending_requests: HashSet<R>,
}

impl<S, R> ManagerCore<S, R>
where
    S: ManagerStateTr,
    R: Eq + Hash,
    R: Eq + Hash + ManagerRequestTr<S>,
{
    /// Create a new info manager core with RPC client
    pub async fn new(rpc_client: Arc<RpcClient>) -> Result<Self> {
        Ok(Self {
            state: S::with_rpc_client(rpc_client.clone()).await?,
            rpc_client,
            pending_requests: HashSet::new(),
        })
    }

    /// Add a pending request to be processed
    fn add_pending_request(&mut self, request: R) {
        self.pending_requests.insert(request);
    }

    /// Process all pending requests
    pub async fn process_pending_requests(&mut self) -> Result<()> {
        let requests = mem::take(&mut self.pending_requests);
        for request in requests {
            if let Err(e) = self.fetch_data(request).await {
                debug!("Error processing request: {}", e);
                // Continue processing other requests even if one fails
            }
        }
        Ok(())
    }

    /// Fetch latest data from the debug server
    ///
    /// This method handles all RPC communication and updates
    /// internal caches that InfoManager instances can read
    async fn fetch_data(&mut self, request: R) -> Result<()> {
        request.fetch_data(self.rpc_client.clone(), &mut self.state).await
    }
}

pub struct ManagerInner<'a, S, R>
where
    S: ManagerStateTr,
    R: Eq + Hash,
    R: Eq + Hash + ManagerRequestTr<S>,
{
    pub(super) core: &'a mut Arc<RwLock<ManagerCore<S, R>>>,
    pub(super) state: &'a mut S,
    pub(super) pending_requests: &'a mut HashSet<R>,
}

pub trait ManagerTr<S, R>
where
    S: ManagerStateTr,
    R: Eq + Hash,
    R: Eq + Hash + ManagerRequestTr<S>,
{
    /// Get Manager inner structure
    fn get_inner<'a>(&'a mut self) -> ManagerInner<'a, S, R>;

    /// Get Manager core
    fn get_core(&self) -> Arc<RwLock<ManagerCore<S, R>>>;

    /// Push pending requests from resolver to core
    async fn push_pending_to_core(&mut self) -> Result<()> {
        let inner = self.get_inner();

        if !inner.pending_requests.is_empty() {
            let requests = mem::take(inner.pending_requests);
            if let Ok(mut core) = inner.core.try_write() {
                for request in requests {
                    core.add_pending_request(request);
                }
            }
        }
        Ok(())
    }

    /// Pull processed data from core to resolver
    fn pull_from_core(&mut self) -> Result<()> {
        let inner = self.get_inner();

        if let Ok(core) = inner.core.try_read() {
            inner.state.update(&core.state);
        }
        Ok(())
    }

    /// Add a new fetching request
    fn new_fetching_request(&mut self, request: R) {
        let inner = self.get_inner();

        if let Ok(mut core) = inner.core.try_write() {
            core.add_pending_request(request);
        } else {
            inner.pending_requests.insert(request);
        }
    }
}
