//! RPC method handlers
//!
//! This module contains all the RPC method implementations for the debug server.

pub mod navigation;

use super::types::{RpcError, RpcId};
use crate::core::EngineContext;
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use std::sync::{atomic::AtomicUsize, Arc, RwLock};
use tracing::debug;

/// Method handler for dispatching RPC calls (runs on single thread)
pub struct MethodHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    /// Complete debugging context
    context: Arc<EngineContext<DB>>,
    /// Current snapshot index
    current_snapshot_index: Arc<AtomicUsize>,
    /// Breakpoints storage
    breakpoints: Arc<RwLock<Vec<super::types::Breakpoint>>>,
}

impl<DB> MethodHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone,
    <CacheDB<DB> as Database>::Error: Clone,
    <DB as Database>::Error: Clone,
{
    pub fn new(
        context: Arc<EngineContext<DB>>,
        current_snapshot_index: Arc<AtomicUsize>,
        breakpoints: Arc<RwLock<Vec<super::types::Breakpoint>>>,
    ) -> Self {
        Self { context, current_snapshot_index, breakpoints }
    }

    /// Handle an RPC method call
    pub async fn handle_method(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, RpcError> {
        debug!("Handling RPC method: {}", method);

        match method {
            // Navigation methods - implemented in navigation.rs
            "debug.getCurrentSnapshot" => {
                navigation::get_current_snapshot(&self.context, &self.current_snapshot_index).await
            }
            "debug.getSnapshotCount" => navigation::get_snapshot_count(&self.context).await,
            "debug.stepNext" => {
                navigation::step_next(&self.context, &self.current_snapshot_index).await
            }
            "debug.stepPrevious" => {
                navigation::step_previous(&self.context, &self.current_snapshot_index).await
            }

            // Unimplemented methods
            _ => Err(RpcError {
                code: -32601,
                message: format!("Method '{}' not found", method),
                data: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_handler_creation() {
        // This test would need a mock EngineContext
        // For now, just test that the module compiles
    }
}
