//! RPC method handlers
//!
//! This module contains all the RPC method implementations for the debug server.

mod trace;

use super::types::RpcError;
use crate::EngineContext;
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use std::sync::Arc;
use tracing::debug;

/// Method handler for dispatching RPC calls (stateless)
pub struct MethodHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    /// Immutable debugging context
    context: Arc<EngineContext<DB>>,
}

impl<DB> MethodHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    pub fn new(context: Arc<EngineContext<DB>>) -> Self {
        Self { context }
    }

    /// Handle an RPC method call with client-provided state
    pub async fn handle_method(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        debug!("Handling RPC method: {}", method);

        match method {
            "edb_getTrace" => trace::get_trace(&self.context),
            // Unimplemented methods
            _ => Err(RpcError {
                code: -32601,
                message: format!("Method '{}' not found", method),
                data: None,
            }),
        }
    }
}
