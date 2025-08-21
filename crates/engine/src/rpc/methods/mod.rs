//! RPC method handlers
//!
//! This module contains all the RPC method implementations for the debug server.

pub mod navigation;

use super::types::RpcError;
use crate::core::EngineContext;
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
            // Navigation methods - implemented in navigation.rs
            "debug.getCurrentSnapshot" => {
                // Extract current_index from params
                let current_index = self.extract_current_index(&params)?;
                navigation::get_current_snapshot(&self.context, current_index).await
            }
            "debug.getSnapshotCount" => navigation::get_snapshot_count(&self.context).await,
            "debug.stepNext" => {
                // Extract current_index from params
                let current_index = self.extract_current_index(&params)?;
                navigation::step_next(&self.context, current_index).await
            }
            "debug.stepPrevious" => {
                // Extract current_index from params
                let current_index = self.extract_current_index(&params)?;
                navigation::step_previous(&self.context, current_index).await
            }
            "debug.setCurrentSnapshot" => {
                // Extract target index from params
                let index = self.extract_snapshot_index(&params)?;
                navigation::set_current_snapshot(&self.context, index).await
            }

            // Unimplemented methods
            _ => Err(RpcError {
                code: -32601,
                message: format!("Method '{}' not found", method),
                data: None,
            }),
        }
    }

    /// Extract current_index from RPC parameters
    fn extract_current_index(&self, params: &Option<serde_json::Value>) -> Result<usize, RpcError> {
        let params = params.as_ref().ok_or_else(|| RpcError {
            code: -32602, // INVALID_PARAMS
            message: "Missing parameters".to_string(),
            data: None,
        })?;

        let index =
            params.get("current_index").and_then(|v| v.as_u64()).map(|v| v as usize).ok_or_else(
                || RpcError {
                    code: -32602, // INVALID_PARAMS
                    message: "Missing or invalid 'current_index' parameter".to_string(),
                    data: None,
                },
            )?;

        Ok(index)
    }

    /// Extract target snapshot index from RPC parameters
    fn extract_snapshot_index(
        &self,
        params: &Option<serde_json::Value>,
    ) -> Result<usize, RpcError> {
        let params = params.as_ref().ok_or_else(|| RpcError {
            code: -32602, // INVALID_PARAMS
            message: "Missing parameters".to_string(),
            data: None,
        })?;

        let index =
            params.get("index").and_then(|v| v.as_u64()).map(|v| v as usize).ok_or_else(|| {
                RpcError {
                    code: -32602, // INVALID_PARAMS
                    message: "Missing or invalid 'index' parameter".to_string(),
                    data: None,
                }
            })?;

        Ok(index)
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
