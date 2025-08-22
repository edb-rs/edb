// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! EDB RPC Proxy Server Library
//!
//! A caching RPC proxy server that sits between EDB components and real Ethereum RPC endpoints.
//! Provides intelligent caching of immutable RPC responses to improve performance and reduce
//! network overhead for multiple debugging sessions.

pub mod cache;
pub mod health;
pub mod metrics;
pub mod providers;
pub mod proxy;
pub mod registry;
pub mod rpc;
pub mod tui;

pub use cache::CacheEntry;
pub use proxy::{ProxyServer, ProxyServerBuilder};
