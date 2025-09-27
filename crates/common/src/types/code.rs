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

use std::{collections::HashMap, path::PathBuf};

use alloy_primitives::Address;
use derive_more::From;
use serde::{Deserialize, Serialize};

/// Represents code information in either opcode or source format for debugging analysis
#[derive(Debug, Clone, Serialize, Deserialize, From)]
pub enum Code {
    /// Opcode-level code representation with disassembled bytecode
    Opcode(#[from] OpcodeInfo),
    /// Source-level code representation with original Solidity source files
    Source(#[from] SourceInfo),
}

impl Code {
    /// Returns the bytecode address, which may differ from the contract address in proxy patterns
    pub fn bytecode_address(&self) -> Address {
        match self {
            Self::Opcode(info) => info.bytecode_address,
            Self::Source(info) => info.bytecode_address,
        }
    }
}

/// Information about disassembled bytecode with opcode mappings for debugging at the EVM level
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpcodeInfo {
    /// The address where the actual bytecode is stored (may differ from address in proxy patterns)
    pub bytecode_address: Address,
    /// Mapping from program counter to disassembled opcode strings for step-by-step debugging
    pub codes: HashMap<usize, String>, // pc -> opcode
}

/// Information about original Solidity source code for high-level debugging with source mappings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceInfo {
    /// The address where the actual bytecode is stored (may differ from address in proxy patterns)
    pub bytecode_address: Address,
    /// Mapping from source file paths to their content for source-level debugging
    pub sources: HashMap<PathBuf, String>, // file -> source
}
