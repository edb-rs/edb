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

    /// Check if this opcode is a call instruction
    fn is_call(&self) -> bool;
}

impl OpcodeTr for OpCode {
    fn modifies_evm_state(&self) -> bool {
        matches!(
            *self,
            // Storage modifications - writes to persistent contract storage
            Self::SSTORE |

            // Account state changes - create/destroy accounts
            Self::CREATE |     // Create new contract account
            Self::CREATE2 |    // Create new contract with deterministic address
            Self::SELFDESTRUCT | // Destroy current contract and transfer balance

            // Balance transfers - modify account balances
            Self::CALL |       // External call that can transfer ETH
            Self::CALLCODE |   // Call with current account context (deprecated)

            // Log emissions - add entries to transaction receipt logs
            Self::LOG0 | Self::LOG1 | Self::LOG2 | Self::LOG3 | Self::LOG4
        )
    }

    fn modifies_transient_storage(&self) -> bool {
        matches!(
            *self,
            Self::TSTORE // Write to transient storage (EIP-1153)
        )
    }

    fn is_call(&self) -> bool {
        matches!(
            *self,
            Self::CREATE
                | Self::CREATE2
                | Self::CALL
                | Self::CALLCODE
                | Self::DELEGATECALL
                | Self::STATICCALL
        )
    }
}
