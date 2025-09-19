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

use foundry_compilers::artifacts::ContractDefinition;
use once_cell::sync::OnceCell;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};

use crate::analysis::macros::universal_id;

universal_id! {
    /// A Universal Contract Identifier (UCID) is a unique identifier for a contract.
    UCID => 0
}

/// A reference-counted pointer to a Contract for efficient sharing across multiple contexts.
#[derive(Debug, Clone)]
pub struct ContractRef {
    inner: Arc<RwLock<Contract>>,
    /* cached readonly fields*/
    name: OnceCell<String>,
    definition: OnceCell<ContractDefinition>,
}

impl From<Contract> for ContractRef {
    fn from(contract: Contract) -> Self {
        Self::new(contract)
    }
}

impl ContractRef {
    /// Creates a new ContractRef from a Contract.
    pub fn new(inner: Contract) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
            name: OnceCell::new(),
            definition: OnceCell::new(),
        }
    }
}

impl ContractRef {
    /// Returns the name of this contract.
    pub fn name(&self) -> &String {
        self.name.get_or_init(|| self.inner.read().definition.name.to_string())
    }

    /// Returns the definition of this contract.
    pub fn definition(&self) -> &ContractDefinition {
        self.definition.get_or_init(|| self.inner.read().definition.clone())
    }
}

#[allow(unused)]
impl ContractRef {
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, Contract> {
        self.inner.read()
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, Contract> {
        self.inner.write()
    }
}

impl Serialize for ContractRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.read().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ContractRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let contract = Contract::deserialize(deserializer)?;
        Ok(Self::new(contract))
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
}
