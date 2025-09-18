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

use std::sync::Arc;

use alloy_dyn_abi::DynSolValue;
use edb_common::types::{parse_callable_abi_entries, CallableAbiEntry, SnapshotInfoDetail};
use eyre::{bail, eyre, Result};
use revm::{database::CacheDB, Database, DatabaseCommit, DatabaseRef};
use tracing::debug;

use super::*;
use crate::{EngineContext, SnapshotDetail};

static EDB_EVAL_PLACEHOLDER_MAGIC: &str = "edb_eval_placeholder";

fn from_abi_info(entry: &CallableAbiEntry) -> Option<DynSolValue> {
    if entry.is_function() {
        return None; // Only handle non-function entries
    }
    let magic = DynSolValue::String(EDB_EVAL_PLACEHOLDER_MAGIC.to_string());
    let serial_abi = DynSolValue::String(serde_json::to_string(entry).ok()?);

    Some(DynSolValue::Tuple(vec![magic, serial_abi]))
}

fn into_abi_info(value: &DynSolValue) -> Option<CallableAbiEntry> {
    if let DynSolValue::Tuple(elements) = value {
        if elements.len() == 2 {
            if let DynSolValue::String(magic) = &elements[0] {
                if magic == EDB_EVAL_PLACEHOLDER_MAGIC {
                    if let DynSolValue::String(serial_abi) = &elements[1] {
                        return serde_json::from_str(serial_abi).ok();
                    }
                }
            }
        }
    }
    None
}

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
            .with_validation_handler(Box::new(EdbValidationHandler(handler.clone())))
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

#[derive(Clone)]
pub struct EdbValidationHandler<DB>(Arc<EdbHandler<DB>>)
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
        let snapshot = &self
            .0
            .context
            .snapshots
            .get(snapshot_id)
            .ok_or_else(|| {
                eyre::eyre!(
                    "Snapshot ID {} not found in EdbHandler::get_variable_value",
                    snapshot_id
                )
            })?
            .1;

        let SnapshotDetail::Hook(detail) = &snapshot.detail() else {
            bail!("Cannot get variable value from opcode snapshot");
        };

        // Let's first check whether this could be a local or state variable
        if let Some(Some(value)) =
            detail.locals.get(name).or_else(|| detail.state_variables.get(name))
        {
            return Ok((**value).clone().into());
        }

        // Next, it might be a mapping/array variable
        let bytecode_address = snapshot.bytecode_address();
        let Some(contract) = self
            .0
            .context
            .recompiled_artifacts
            .get(&bytecode_address)
            .and_then(|art| art.contract())
        else {
            bail!("No contract found for bytecode address {:?}", bytecode_address);
        };

        for entry in parse_callable_abi_entries(contract) {
            if entry.name == name && entry.is_state_variable() {
                if let Some(value) = from_abi_info(&entry) {
                    return Ok(value);
                }
            }
        }

        bail!("No value found for name='{}', snapshot_id={}", name, snapshot_id)
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
        if let Some(abi_info) = into_abi_info(&root) {
            let (_, info) = self.0.context.snapshots.get(snapshot_id).ok_or_else(|| {
                eyre::eyre!(
                    "Snapshot ID {} not found in EdbHandler::get_mapping_or_array_value",
                    snapshot_id
                )
            })?;
            let to = info.target_address();

            self.0.context.call_in_derived_evm(snapshot_id, to, &abi_info.abi, &indices, None)
        } else {
            // Handle direct DynSolValue types recursively
            if indices.is_empty() {
                return Ok(root);
            }

            let (first_index, remaining_indices) = indices.split_first().unwrap();

            let next_value = match &root {
                DynSolValue::Tuple(elements) => {
                    // For tuples, index should be a uint
                    match first_index {
                        DynSolValue::Uint(idx, _) => {
                            let idx = idx.to::<usize>();
                            if idx >= elements.len() {
                                bail!(
                                    "Index {} out of bounds for tuple with {} elements",
                                    idx,
                                    elements.len()
                                );
                            }
                            elements[idx].clone()
                        }
                        _ => bail!(
                            "Invalid index type for tuple access: expected uint, got {:?}",
                            first_index
                        ),
                    }
                }
                DynSolValue::Array(elements) => {
                    // For arrays, index should be a uint
                    match first_index {
                        DynSolValue::Uint(idx, _) => {
                            let idx = idx.to::<usize>();
                            if idx >= elements.len() {
                                bail!(
                                    "Index {} out of bounds for array with {} elements",
                                    idx,
                                    elements.len()
                                );
                            }
                            elements[idx].clone()
                        }
                        _ => bail!(
                            "Invalid index type for array access: expected uint, got {:?}",
                            first_index
                        ),
                    }
                }
                DynSolValue::FixedArray(elements) => {
                    // For fixed arrays, index should be a uint
                    match first_index {
                        DynSolValue::Uint(idx, _) => {
                            let idx = idx.to::<usize>();
                            if idx >= elements.len() {
                                bail!(
                                    "Index {} out of bounds for fixed array with {} elements",
                                    idx,
                                    elements.len()
                                );
                            }
                            elements[idx].clone()
                        }
                        _ => bail!(
                            "Invalid index type for fixed array access: expected uint, got {:?}",
                            first_index
                        ),
                    }
                }
                _ => {
                    bail!(
                        "Cannot index into value of type {:?} with index {:?}",
                        root,
                        first_index
                    );
                }
            };

            // Recursively handle remaining indices - this is key because next_value might have abi_info
            self.get_mapping_or_array_value(next_value, remaining_indices.to_vec(), snapshot_id)
        }
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
        debug!(
            "EdbHandler::call_function name='{}', args={:?}, callee={:?}, snapshot_id={}",
            name, args, callee, snapshot_id
        );

        let (frame_id, snapshot) = self.0.context.snapshots.get(snapshot_id).ok_or_else(|| {
            eyre::eyre!("Snapshot ID {} not found in EdbHandler::call_function", snapshot_id)
        })?;

        // Let's first handle our edb-specific pseudo-functions
        if name == "edb_sload" && args.len() == 2 {
            match (&args[0], &args[1]) {
                (DynSolValue::Address(address), DynSolValue::Uint(slot, ..)) => {
                    let db = snapshot.db();
                    let cached_storage = db
                        .cache
                        .accounts
                        .get(address)
                        .map(|acc| &acc.storage)
                        .ok_or(eyre!("Account {:?} not found in edb_sload", address))?;
                    let value = cached_storage.get(slot).cloned().unwrap_or_default();

                    return Ok(DynSolValue::Uint(value, 256));
                }
                _ => {
                    bail!(
                        "Invalid arguments to edb_sload: expected (string, u256), got ({:?}, {:?})",
                        args[0],
                        args[1]
                    );
                }
            }
        }

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

impl<DB> ValidationHandler for EdbValidationHandler<DB>
where
    DB: Database + DatabaseCommit + DatabaseRef + Clone + Send + Sync + 'static,
    <CacheDB<DB> as Database>::Error: Clone + Send + Sync,
    <DB as Database>::Error: Clone + Send + Sync,
{
    fn validate_value(&self, value: DynSolValue) -> Result<DynSolValue> {
        if into_abi_info(&value).is_some() {
            bail!("Mapping or array value cannot be directly returned; please access a member or call a function to get a concrete value");
        } else {
            Ok(value)
        }
    }
}
