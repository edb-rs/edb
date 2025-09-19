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

// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0

//! EDB Engine - Advanced Ethereum smart contract debugging and analysis engine.
//!
//! This crate provides the core debugging engine for EDB (Ethereum Debugger), enabling
//! detailed analysis and debugging of smart contract execution at both opcode and source levels.
//! The engine supports real-time expression evaluation, comprehensive snapshot management,
//! and advanced debugging features for Ethereum smart contracts.
//!
//! # Key Features
//!
//! ## 🔍 **Multi-Level Debugging**
//! - **Opcode-level debugging**: Step through EVM bytecode execution
//! - **Source-level debugging**: Debug using original Solidity source code
//! - **Hybrid debugging**: Switch seamlessly between opcode and source views
//!
//! ## 📸 **Snapshot System**
//! - Capture execution state at any point during contract execution
//! - Navigate forward/backward through execution history
//! - Support for both hook-based and opcode-based snapshots
//!
//! ## ⚡ **Real-time Expression Evaluation**
//! - Evaluate Solidity-like expressions against any execution snapshot
//! - Access variables, function calls, storage, and blockchain context
//! - Support for complex expressions with type casting and operations
//!
//! ## 🔗 **JSON-RPC API**
//! - Complete debugging API for frontend integration
//! - WebSocket and HTTP support for real-time debugging
//! - Comprehensive error handling and protocol compliance
//!
//! ## 🛠 **Advanced Analysis**
//! - Contract instrumentation for debugging hooks
//! - Source code analysis and mapping
//! - ABI resolution and type inference
//! - Storage layout analysis
//!
//! # Core Modules
//!
//! - [`analysis`] - Source code analysis and contract instrumentation
//! - [`core`] - Core engine types and execution management
//! - [`context`] - Engine context and state management
//! - [`eval`] - Expression evaluation system
//! - [`inspector`] - EVM execution inspectors for data collection
//! - [`instrumentation`] - Contract instrumentation and code generation
//! - [`rpc`] - JSON-RPC debugging API
//! - [`snapshot`] - Snapshot management and analysis
//! - [`tweak`] - Runtime contract modification for debugging
//! - [`utils`] - Utility functions and helpers
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use edb_engine::{EngineContext, DebugRpcServer};
//! use revm::primitives::Address;
//!
//! // Create engine context with debugging data
//! let context = EngineContext::new(db, snapshots, trace, artifacts);
//!
//! // Start RPC server for debugging API
//! let server = DebugRpcServer::new(context);
//! let handle = server.start().await?;
//!
//! println!("Debug server running on port {}", handle.port());
//! ```
//!
//! # Architecture
//!
//! The EDB engine follows a modular architecture:
//!
//! 1. **Data Collection**: Inspectors collect execution data during EVM execution
//! 2. **Snapshot Management**: Captured data is organized into navigable snapshots
//! 3. **Expression Evaluation**: Real-time evaluation against snapshot data
//! 4. **API Layer**: JSON-RPC interface exposes all debugging functionality
//! 5. **Analysis**: Deep contract analysis for enhanced debugging experience

/// Source code analysis and contract instrumentation (internal module).
pub mod analysis;
use analysis::*;

pub mod core;
pub use core::*;

pub mod context;
pub use context::*;

pub mod eval;
pub use eval::*;

pub mod inspector;
pub use inspector::*;

/// Contract instrumentation and code generation (internal module).
pub mod instrumentation;
pub use instrumentation::*;

pub mod rpc;
pub use rpc::*;

pub mod snapshot;
pub use snapshot::*;

pub mod tweak;
pub use tweak::*;

pub mod utils;
pub use utils::*;
