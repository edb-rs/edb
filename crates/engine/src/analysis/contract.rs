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

use foundry_compilers::artifacts::ContractDefinition;
use serde::{Deserialize, Serialize};

use crate::analysis::macros::{define_ref, universal_id};

universal_id! {
    /// A Universal Contract Identifier (UCID) is a unique identifier for a contract.
    UCID => 0
}

define_ref! {
    /// A reference-counted pointer to a Contract for efficient sharing across multiple contexts.
    ContractRef(Contract) {
        cached_field: {
            definition: ContractDefinition,
        }
        cached_method: {
            name: String,
        }
    }
}
/// Represents a contract in a smart contract with its metadata and type information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    /// The unique contract identifier.
    pub ucid: UCID,
    /// The contract definition.
    pub definition: ContractDefinition,
}

impl Contract {
    /// Creates a new Contract with the given definition.
    pub fn new(definition: ContractDefinition) -> Self {
        Self { ucid: UCID::next(), definition }
    }

    /// Returns the name of this contract.
    pub fn name(&self) -> String {
        self.definition.name.to_string()
    }
}
