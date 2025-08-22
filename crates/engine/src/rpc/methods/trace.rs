use std::sync::Arc;

use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};

use crate::{EngineContext, RpcError};

pub fn get_trace<DB>(context: &Arc<EngineContext<DB>>) -> Result<serde_json::Value, RpcError>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    // Implementation goes here
    let trace = &context.trace;
    Ok(serde_json::json!(trace))
}
