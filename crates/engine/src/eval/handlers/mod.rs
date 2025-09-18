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

use alloy_dyn_abi::DynSolValue;
use eyre::Result;

/// Handler trait for variable value resolution
pub trait VariableHandler {
    fn get_variable_value(&self, name: &str, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Handler trait for mapping and array access
pub trait MappingArrayHandler {
    fn get_mapping_or_array_value(
        &self,
        root: DynSolValue,
        indices: Vec<DynSolValue>,
        snapshot_id: usize,
    ) -> Result<DynSolValue>;
}

/// Handler trait for function calls
pub trait FunctionCallHandler {
    fn call_function(
        &self,
        name: &str,
        args: &[DynSolValue],
        callee: Option<&DynSolValue>,
        snapshot_id: usize,
    ) -> Result<DynSolValue>;
}

/// Handler trait for member access (e.g., struct fields, array.length)
pub trait MemberAccessHandler {
    fn access_member(
        &self,
        value: DynSolValue,
        member: &str,
        snapshot_id: usize,
    ) -> Result<DynSolValue>;
}

/// Handler trait for msg global variables
pub trait MsgHandler {
    fn get_msg_sender(&self, snapshot_id: usize) -> Result<DynSolValue>;
    fn get_msg_value(&self, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Handler trait for tx global variables
pub trait TxHandler {
    fn get_tx_origin(&self, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Handler trait for block global variables
pub trait BlockHandler {
    fn get_block_number(&self, snapshot_id: usize) -> Result<DynSolValue>;
    fn get_block_timestamp(&self, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Combined handlers struct for the evaluator
#[derive(Default)]
pub struct EvaluatorHandlers {
    pub variable_handler: Option<Box<dyn VariableHandler>>,
    pub mapping_array_handler: Option<Box<dyn MappingArrayHandler>>,
    pub function_call_handler: Option<Box<dyn FunctionCallHandler>>,
    pub member_access_handler: Option<Box<dyn MemberAccessHandler>>,
    pub msg_handler: Option<Box<dyn MsgHandler>>,
    pub tx_handler: Option<Box<dyn TxHandler>>,
    pub block_handler: Option<Box<dyn BlockHandler>>,
}

impl Clone for EvaluatorHandlers {
    fn clone(&self) -> Self {
        // Note: This creates a new empty handlers struct
        // If handlers need to be cloned with their state, they would need to implement a clone method
        Self::default()
    }
}

impl EvaluatorHandlers {
    /// Create new empty handlers
    pub fn new() -> Self {
        Self::default()
    }

    /// Set variable handler
    pub fn with_variable_handler(mut self, handler: Box<dyn VariableHandler>) -> Self {
        self.variable_handler = Some(handler);
        self
    }

    /// Set mapping/array handler
    pub fn with_mapping_array_handler(mut self, handler: Box<dyn MappingArrayHandler>) -> Self {
        self.mapping_array_handler = Some(handler);
        self
    }

    /// Set function call handler
    pub fn with_function_call_handler(mut self, handler: Box<dyn FunctionCallHandler>) -> Self {
        self.function_call_handler = Some(handler);
        self
    }

    /// Set member access handler
    pub fn with_member_access_handler(mut self, handler: Box<dyn MemberAccessHandler>) -> Self {
        self.member_access_handler = Some(handler);
        self
    }

    /// Set msg handler
    pub fn with_msg_handler(mut self, handler: Box<dyn MsgHandler>) -> Self {
        self.msg_handler = Some(handler);
        self
    }

    /// Set tx handler
    pub fn with_tx_handler(mut self, handler: Box<dyn TxHandler>) -> Self {
        self.tx_handler = Some(handler);
        self
    }

    /// Set block handler
    pub fn with_block_handler(mut self, handler: Box<dyn BlockHandler>) -> Self {
        self.block_handler = Some(handler);
        self
    }
}

pub mod debug;
pub mod edb;
