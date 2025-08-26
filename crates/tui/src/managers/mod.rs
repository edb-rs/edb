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

//! Shared state managers for TUI panels
//!
//! This module contains managers that handle shared state between panels.

use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Deref, DerefMut},
};

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

pub mod execution;
pub mod resolve;
pub mod theme;

pub use execution::ExecutionManagerCore;
pub use resolve::ResolverCore;
pub use theme::ThemeManagerCore;
