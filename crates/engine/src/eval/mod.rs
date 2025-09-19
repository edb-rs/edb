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

//! Expression evaluation system for EDB.
//!
//! This module provides a comprehensive expression evaluation system that enables
//! real-time evaluation of Solidity-like expressions against debug snapshots.
//! It supports variables, function calls, arithmetic operations, and blockchain context.
//!
//! # Main Components
//!
//! - [`ExpressionEvaluator`] - Main evaluator for parsing and executing expressions
//! - [`handlers`] - Handler traits and implementations for different evaluation contexts
//! - Common types and utilities for expression evaluation
//!
//! # Basic Usage
//!
//! ```rust,ignore
//! use edb_engine::eval::{ExpressionEvaluator, handlers::EdbHandler};
//!
//! // Create evaluator with EDB handlers
//! let handlers = EdbHandler::create_handlers(engine_context);
//! let evaluator = ExpressionEvaluator::new(handlers);
//!
//! // Evaluate expressions
//! let result = evaluator.eval("balances[msg.sender]", snapshot_id)?;
//! let result = evaluator.eval("totalSupply() > 1000000", snapshot_id)?;
//! let result = evaluator.eval("block.timestamp - lastUpdate > 3600", snapshot_id)?;
//! ```
//!
//! # Supported Expressions
//!
//! - **Variables**: `balance`, `owner`, `this`
//! - **Mappings/Arrays**: `balances[addr]`, `users[0]`
//! - **Function Calls**: `balanceOf(user)`, `totalSupply()`
//! - **Member Access**: `token.symbol`, `addr.balance`
//! - **Arithmetic**: `+`, `-`, `*`, `/`, `%`, `**`
//! - **Comparison**: `==`, `!=`, `<`, `<=`, `>`, `>=`
//! - **Logical**: `&&`, `||`, `!`
//! - **Ternary**: `condition ? true_value : false_value`
//! - **Type Casting**: `uint256(value)`, `address(0x123...)`
//! - **Blockchain Context**: `msg.sender`, `msg.value`, `block.number`, `tx.origin`

mod common;
pub use common::*;

mod evaluator;
pub mod handlers;
mod utils;

pub use evaluator::ExpressionEvaluator;
