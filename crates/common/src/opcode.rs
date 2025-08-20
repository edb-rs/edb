// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
// SPDX-License-Identifier: AGPL-3.0
//! Extended opcode analysis utilities for EVM state modification detection
//!
//! This module provides additional methods for analyzing EVM opcodes beyond
//! what's available in the base revm library, particularly focused on
//! detecting various types of state modifications that are important for
//! debugging and analysis tools.

use revm::bytecode::OpCode;

/// Extended trait for EVM opcode analysis
///
/// Provides additional methods for determining what types of EVM state
/// an opcode will modify when executed. This is useful for:
/// - Debugging tools that need to track state changes
/// - Optimization of state snapshots and memory sharing
/// - Security analysis of contract execution
pub trait OpcodeTr {
    /// Check if this opcode modifies persistent EVM state
    ///
    /// Returns `true` if the opcode will modify any persistent state including:
    /// - Storage (via SSTORE)
    /// - Account state (via CREATE, CREATE2, SELFDESTRUCT)
    /// - Account balances (via CALL, CALLCODE with value transfer)
    /// - Event logs (via LOG0-LOG4)
    ///
    /// Note: This does NOT include temporary state like memory or stack,
    /// only state that persists beyond the current transaction execution.
    ///
    /// # Example
    /// ```rust
    /// use revm::bytecode::OpCode;
    /// use edb_common::OpcodeTr;
    ///
    /// assert!(OpCode::SSTORE.modifies_evm_state());
    /// assert!(OpCode::CREATE.modifies_evm_state());
    /// assert!(!OpCode::ADD.modifies_evm_state());
    /// assert!(!OpCode::MSTORE.modifies_evm_state()); // Only modifies memory
    /// ```
    fn modifies_evm_state(&self) -> bool;

    /// Check if this opcode modifies transient storage (EIP-1153)
    ///
    /// Transient storage is temporary storage that exists only for the
    /// duration of a transaction and is cleared when the transaction ends.
    ///
    /// Returns `true` only for:
    /// - `TSTORE` (0x5C): Write to transient storage
    ///
    /// Note: `TLOAD` (0x5D) only reads transient storage and doesn't modify it.
    ///
    /// # Example
    /// ```rust
    /// use revm::bytecode::OpCode;
    /// use edb_common::OpcodeTr;
    ///
    /// assert!(OpCode::TSTORE.modifies_transient_storage());
    /// assert!(!OpCode::TLOAD.modifies_transient_storage());
    /// assert!(!OpCode::SSTORE.modifies_transient_storage());
    /// ```
    fn modifies_transient_storage(&self) -> bool;
}

impl OpcodeTr for OpCode {
    fn modifies_evm_state(&self) -> bool {
        matches!(
            *self,
            // Storage modifications - writes to persistent contract storage
            OpCode::SSTORE |

            // Account state changes - create/destroy accounts
            OpCode::CREATE |     // Create new contract account
            OpCode::CREATE2 |    // Create new contract with deterministic address
            OpCode::SELFDESTRUCT | // Destroy current contract and transfer balance

            // Balance transfers - modify account balances
            OpCode::CALL |       // External call that can transfer ETH
            OpCode::CALLCODE |   // Call with current account context (deprecated)

            // Log emissions - add entries to transaction receipt logs
            OpCode::LOG0 | OpCode::LOG1 | OpCode::LOG2 | OpCode::LOG3 | OpCode::LOG4
        )
    }

    fn modifies_transient_storage(&self) -> bool {
        matches!(
            *self,
            OpCode::TSTORE // Write to transient storage (EIP-1153)
        )
    }
}
