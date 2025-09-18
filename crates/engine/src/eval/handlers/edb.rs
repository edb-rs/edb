use std::sync::Arc;

use alloy_dyn_abi::DynSolValue;
use eyre::{bail, Result};
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};

use super::*;
use crate::EngineContext;

/// EDB-specific handler that uses EdbContext to resolve values
pub struct EdbHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    context: Arc<EngineContext<DB>>,
}

impl<DB> EdbHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    pub fn new(context: Arc<EngineContext<DB>>) -> Self {
        Self { context }
    }

    /// Create all handlers using this EDB context
    pub fn create_handlers(context: Arc<EngineContext<DB>>) -> EvaluatorHandlers {
        let handler = Arc::new(Self::new(context));

        EvaluatorHandlers::new()
            .with_variable_handler(Box::new(EdbVariableHandler(handler.clone())))
            .with_mapping_array_handler(Box::new(EdbMappingArrayHandler(handler.clone())))
            .with_function_call_handler(Box::new(EdbFunctionCallHandler(handler.clone())))
            .with_member_access_handler(Box::new(EdbMemberAccessHandler(handler.clone())))
            .with_msg_handler(Box::new(EdbMsgHandler(handler.clone())))
            .with_tx_handler(Box::new(EdbTxHandler(handler.clone())))
            .with_block_handler(Box::new(EdbBlockHandler(handler.clone())))
    }
}

// Wrapper structs for each handler trait
#[derive(Clone)]
pub struct EdbVariableHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

#[derive(Clone)]
pub struct EdbMappingArrayHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

#[derive(Clone)]
pub struct EdbFunctionCallHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

#[derive(Clone)]
pub struct EdbMemberAccessHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

#[derive(Clone)]
pub struct EdbMsgHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

#[derive(Clone)]
pub struct EdbTxHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

#[derive(Clone)]
pub struct EdbBlockHandler<DB>(Arc<EdbHandler<DB>>)
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync;

// Implement handler traits for each wrapper
impl<DB> VariableHandler for EdbVariableHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn get_variable_value(&self, name: &str, snapshot_id: usize) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to fetch variable value from snapshot
        bail!(
            "EdbHandler::get_variable_value not yet implemented for name='{}', snapshot_id={}",
            name,
            snapshot_id
        )
    }
}

impl<DB> MappingArrayHandler for EdbMappingArrayHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn get_mapping_or_array_value(
        &self,
        root: DynSolValue,
        indices: Vec<DynSolValue>,
        snapshot_id: usize,
    ) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to fetch mapping/array value from snapshot
        bail!("EdbHandler::get_mapping_or_array_value not yet implemented for root={:?}, indices={:?}, snapshot_id={}", root, indices, snapshot_id)
    }
}

impl<DB> FunctionCallHandler for EdbFunctionCallHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn call_function(
        &self,
        name: &str,
        args: &[DynSolValue],
        callee: Option<&DynSolValue>,
        snapshot_id: usize,
    ) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to call function in snapshot
        bail!("EdbHandler::call_function not yet implemented for name='{}', args={:?}, callee={:?}, snapshot_id={}", name, args, callee, snapshot_id)
    }
}

impl<DB> MemberAccessHandler for EdbMemberAccessHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn access_member(
        &self,
        value: DynSolValue,
        member: &str,
        snapshot_id: usize,
    ) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to access member in snapshot
        bail!("EdbHandler::access_member not yet implemented for value={:?}, member='{}', snapshot_id={}", value, member, snapshot_id)
    }
}

impl<DB> MsgHandler for EdbMsgHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn get_msg_sender(&self, snapshot_id: usize) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to get msg.sender from snapshot
        bail!("EdbHandler::get_msg_sender not yet implemented for snapshot_id={}", snapshot_id)
    }

    fn get_msg_value(&self, snapshot_id: usize) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to get msg.value from snapshot
        bail!("EdbHandler::get_msg_value not yet implemented for snapshot_id={}", snapshot_id)
    }
}

impl<DB> TxHandler for EdbTxHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn get_tx_origin(&self, snapshot_id: usize) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to get tx.origin from snapshot
        bail!("EdbHandler::get_tx_origin not yet implemented for snapshot_id={}", snapshot_id)
    }
}

impl<DB> BlockHandler for EdbBlockHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn get_block_number(&self, snapshot_id: usize) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to get block.number from snapshot
        bail!("EdbHandler::get_block_number not yet implemented for snapshot_id={}", snapshot_id)
    }

    fn get_block_timestamp(&self, snapshot_id: usize) -> Result<DynSolValue> {
        // TODO: Implement using self.0.context to get block.timestamp from snapshot
        bail!("EdbHandler::get_block_timestamp not yet implemented for snapshot_id={}", snapshot_id)
    }
}
