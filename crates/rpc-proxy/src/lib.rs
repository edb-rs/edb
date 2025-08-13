//! EDB RPC Proxy Server Library
//!
//! A caching RPC proxy server that sits between EDB components and real Ethereum RPC endpoints.
//! Provides intelligent caching of immutable RPC responses to improve performance and reduce
//! network overhead for multiple debugging sessions.

pub mod cache;
pub mod health;
pub mod proxy;
pub mod registry;
pub mod rpc;
