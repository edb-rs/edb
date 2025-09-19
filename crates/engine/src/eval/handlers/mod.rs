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

//! Handler traits and types for the expression evaluator.
//!
//! This module defines the handler traits that allow the expression evaluator to
//! interact with different data sources and execution contexts. The handler pattern
//! enables flexible evaluation of Solidity-like expressions against various backends
//! such as debug snapshots, EVM state, or simulation environments.
//!
//! # Handler Types
//!
//! - [`VariableHandler`] - Resolves variable values by name
//! - [`MappingArrayHandler`] - Handles mapping and array access operations
//! - [`FunctionCallHandler`] - Executes function calls and built-in functions
//! - [`MemberAccessHandler`] - Resolves member access on values (e.g., struct fields)
//! - [`MsgHandler`] - Provides transaction context (`msg.sender`, `msg.value`)
//! - [`TxHandler`] - Provides transaction globals (`tx.origin`)
//! - [`BlockHandler`] - Provides block context (`block.number`, `block.timestamp`)
//! - [`ValidationHandler`] - Validates final expression results
//!
//! # Usage
//!
//! Handlers are typically used through [`EvaluatorHandlers`], which aggregates
//! all handler types and can be configured with different implementations:
//!
//! ```rust,ignore
//! let handlers = EvaluatorHandlers::new()
//!     .with_variable_handler(Box::new(my_variable_handler))
//!     .with_function_call_handler(Box::new(my_function_handler));
//! ```

use alloy_dyn_abi::DynSolValue;
use eyre::Result;

/// Handler trait for variable value resolution.
///
/// Implementations resolve variable names to their values in a specific execution context.
/// This includes local variables, state variables, and special variables like `this`.
pub trait VariableHandler {
    /// Get the value of a variable by name.
    ///
    /// # Arguments
    /// * `name` - The variable name to resolve
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The variable's value as a [`DynSolValue`], or an error if the variable doesn't exist
    fn get_variable_value(&self, name: &str, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Handler trait for mapping and array access operations.
///
/// Handles indexing operations like `mapping[key]`, `array[index]`, and nested access
/// like `mapping[key1][key2]`. Supports both storage-based mappings and in-memory arrays.
pub trait MappingArrayHandler {
    /// Access a mapping or array with the given indices.
    ///
    /// # Arguments
    /// * `root` - The base value being indexed (mapping, array, or ABI info)
    /// * `indices` - Vector of index values for nested access
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The value at the specified indices, or an error if access fails
    fn get_mapping_or_array_value(
        &self,
        root: DynSolValue,
        indices: Vec<DynSolValue>,
        snapshot_id: usize,
    ) -> Result<DynSolValue>;
}

/// Handler trait for function calls and built-in functions.
///
/// Executes function calls including contract functions, built-in functions (like `keccak256`),
/// and EDB-specific functions (like `edb_sload`).
pub trait FunctionCallHandler {
    /// Call a function with the given arguments.
    ///
    /// # Arguments
    /// * `name` - The function name to call
    /// * `args` - Function arguments as [`DynSolValue`] array
    /// * `callee` - Optional target address for the call (None uses current context)
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The function's return value, or an error if the call fails
    fn call_function(
        &self,
        name: &str,
        args: &[DynSolValue],
        callee: Option<&DynSolValue>,
        snapshot_id: usize,
    ) -> Result<DynSolValue>;
}

/// Handler trait for member access operations.
///
/// Handles dot notation access like `struct.field`, `array.length`, or `address.balance`.
/// Works with both storage state and computed properties.
pub trait MemberAccessHandler {
    /// Access a member of the given value.
    ///
    /// # Arguments
    /// * `value` - The base value whose member is being accessed
    /// * `member` - The member name to access
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The member's value, or an error if the member doesn't exist
    fn access_member(
        &self,
        value: DynSolValue,
        member: &str,
        snapshot_id: usize,
    ) -> Result<DynSolValue>;
}

/// Handler trait for `msg` global variables.
///
/// Provides access to transaction context variables like `msg.sender` and `msg.value`.
pub trait MsgHandler {
    /// Get the message sender address (`msg.sender`).
    ///
    /// # Arguments
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The sender address as a [`DynSolValue::Address`]
    fn get_msg_sender(&self, snapshot_id: usize) -> Result<DynSolValue>;

    /// Get the message value (`msg.value`).
    ///
    /// # Arguments
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The message value in wei as a [`DynSolValue::Uint`]
    fn get_msg_value(&self, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Handler trait for `tx` global variables.
///
/// Provides access to transaction-level context like `tx.origin`.
pub trait TxHandler {
    /// Get the transaction origin address (`tx.origin`).
    ///
    /// # Arguments
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The origin address as a [`DynSolValue::Address`]
    fn get_tx_origin(&self, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Handler trait for final expression validation.
///
/// Validates and potentially transforms the final result of expression evaluation.
/// Used to enforce constraints or convert placeholder values to concrete values.
pub trait ValidationHandler {
    /// Validate and potentially transform a final expression value.
    ///
    /// # Arguments
    /// * `value` - The value to validate
    ///
    /// # Returns
    /// The validated (possibly transformed) value, or an error if invalid
    fn validate_value(&self, value: DynSolValue) -> Result<DynSolValue>;
}

/// Handler trait for `block` global variables.
///
/// Provides access to blockchain context like block number and timestamp.
pub trait BlockHandler {
    /// Get the current block number (`block.number`).
    ///
    /// # Arguments
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The block number as a [`DynSolValue::Uint`]
    fn get_block_number(&self, snapshot_id: usize) -> Result<DynSolValue>;

    /// Get the current block timestamp (`block.timestamp`).
    ///
    /// # Arguments
    /// * `snapshot_id` - The execution context identifier
    ///
    /// # Returns
    /// The block timestamp as a [`DynSolValue::Uint`]
    fn get_block_timestamp(&self, snapshot_id: usize) -> Result<DynSolValue>;
}

/// Combined handlers struct for the expression evaluator.
///
/// Aggregates all handler types needed for expression evaluation. Each handler
/// is optional, allowing for partial implementations where only certain features
/// are needed.
///
/// # Example
///
/// ```rust,ignore
/// let handlers = EvaluatorHandlers::new()
///     .with_variable_handler(Box::new(MyVariableHandler))
///     .with_function_call_handler(Box::new(MyFunctionHandler))
///     .with_msg_handler(Box::new(MyMsgHandler));
/// ```
#[derive(Default)]
pub struct EvaluatorHandlers {
    /// Handler for variable resolution
    pub variable_handler: Option<Box<dyn VariableHandler>>,
    /// Handler for mapping and array access
    pub mapping_array_handler: Option<Box<dyn MappingArrayHandler>>,
    /// Handler for function calls
    pub function_call_handler: Option<Box<dyn FunctionCallHandler>>,
    /// Handler for member access operations
    pub member_access_handler: Option<Box<dyn MemberAccessHandler>>,
    /// Handler for `msg` global variables
    pub msg_handler: Option<Box<dyn MsgHandler>>,
    /// Handler for `tx` global variables
    pub tx_handler: Option<Box<dyn TxHandler>>,
    /// Handler for `block` global variables
    pub block_handler: Option<Box<dyn BlockHandler>>,
    /// Handler for final value validation
    pub validation_handler: Option<Box<dyn ValidationHandler>>,
}

impl Clone for EvaluatorHandlers {
    /// Clone the handlers struct.
    ///
    /// Note: This creates a new empty handlers struct since trait objects
    /// cannot be cloned. If handlers need to be cloned with their state,
    /// they would need to implement a custom clone method.
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl EvaluatorHandlers {
    /// Create a new empty handlers collection.
    ///
    /// All handlers are initially `None` and can be set using the `with_*` methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the variable handler.
    ///
    /// # Arguments
    /// * `handler` - The variable handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_variable_handler(mut self, handler: Box<dyn VariableHandler>) -> Self {
        self.variable_handler = Some(handler);
        self
    }

    /// Set the mapping/array handler.
    ///
    /// # Arguments
    /// * `handler` - The mapping/array handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_mapping_array_handler(mut self, handler: Box<dyn MappingArrayHandler>) -> Self {
        self.mapping_array_handler = Some(handler);
        self
    }

    /// Set the function call handler.
    ///
    /// # Arguments
    /// * `handler` - The function call handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_function_call_handler(mut self, handler: Box<dyn FunctionCallHandler>) -> Self {
        self.function_call_handler = Some(handler);
        self
    }

    /// Set the member access handler.
    ///
    /// # Arguments
    /// * `handler` - The member access handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_member_access_handler(mut self, handler: Box<dyn MemberAccessHandler>) -> Self {
        self.member_access_handler = Some(handler);
        self
    }

    /// Set the msg handler.
    ///
    /// # Arguments
    /// * `handler` - The msg handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_msg_handler(mut self, handler: Box<dyn MsgHandler>) -> Self {
        self.msg_handler = Some(handler);
        self
    }

    /// Set the tx handler.
    ///
    /// # Arguments
    /// * `handler` - The tx handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_tx_handler(mut self, handler: Box<dyn TxHandler>) -> Self {
        self.tx_handler = Some(handler);
        self
    }

    /// Set the block handler.
    ///
    /// # Arguments
    /// * `handler` - The block handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_block_handler(mut self, handler: Box<dyn BlockHandler>) -> Self {
        self.block_handler = Some(handler);
        self
    }

    /// Set the validation handler.
    ///
    /// # Arguments
    /// * `handler` - The validation handler implementation
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_validation_handler(mut self, handler: Box<dyn ValidationHandler>) -> Self {
        self.validation_handler = Some(handler);
        self
    }
}

/// Debug handler implementations for testing and simulation
pub mod debug;
/// EDB handler implementations for real debug snapshots
pub mod edb;
