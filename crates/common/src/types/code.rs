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

#[derive(Debug, Clone, Serialize, Deserialize, From)]
pub enum Code {
    Opcode(#[from] OpcodeInfo),
    Source(#[from] SourceInfo),
}

impl Code {
    pub fn address(&self) -> Address {
        match self {
            Code::Opcode(info) => info.address,
            Code::Source(info) => info.address,
        }
    }

    pub fn bytecode_address(&self) -> Address {
        match self {
            Code::Opcode(info) => info.bytecode_address,
            Code::Source(info) => info.bytecode_address,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpcodeInfo {
    pub address: Address,
    pub bytecode_address: Address,
    pub codes: HashMap<u64, String>, // pc -> opcode
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceInfo {
    pub address: Address,
    pub bytecode_address: Address,
    pub sources: HashMap<PathBuf, String>, // file -> source
}
