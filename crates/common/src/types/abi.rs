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

use std::{borrow::Borrow, fmt};

use alloy_json_abi::Function;
use alloy_primitives::Address;
use foundry_compilers::artifacts::Contract;
use serde::{Deserialize, Serialize};
use tracing::error;

/// Magic flag to identify state variables in the instrumented code.
pub static EDB_STATE_VAR_FLAG: &str = "_edb_state_var_";

/// The type of an callable ABI entry from outside the contract.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AbiEntryTy {
    /// A function that can be called.
    Function,
    /// A state variable that can be read.
    StateVariable(u64),
}

/// Information about a callable entry in an ABI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallableAbiEntry {
    /// The name of the callable entry.
    pub name: String,
    /// The type of the callable entry.
    pub ty: AbiEntryTy,
    /// The input types of the callable entry.
    pub inputs: Vec<String>,
    /// The output types of the callable entry.
    pub outputs: Vec<String>,
    /// The actually function abi
    pub abi: Function,
}

/// Contract type in terms of proxy pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ContractTy {
    /// A normal contract.
    Normal,
    /// A proxy contract.
    Proxy,
    /// An implementation contract.
    Implementation,
}

/// Information about all callable ABI entries of a contract.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallableAbiInfo {
    /// The address of the contract.
    pub address: Address,
    /// The type of the address (normal, proxy, implementation).
    pub contract_ty: ContractTy,
    /// The callable ABI entries of the contract.
    pub entries: Vec<CallableAbiEntry>,
}

impl<T> From<T> for CallableAbiEntry
where
    T: Borrow<Function>,
{
    fn from(function: T) -> Self {
        let mut name = function.borrow().name.clone();

        if let Some(pos) = name.find(EDB_STATE_VAR_FLAG) {
            let uvid = name[pos + EDB_STATE_VAR_FLAG.len()..].parse::<u64>().unwrap_or_else(|e| {
                error!(
                    error=?e,
                    "Failed to parse state variable ID from function name: {}",
                    name
                );
                0
            });
            name.truncate(pos);
            Self {
                name,
                ty: AbiEntryTy::StateVariable(uvid),
                inputs: function.borrow().inputs.iter().map(|i| i.ty.clone()).collect(),
                outputs: function.borrow().outputs.iter().map(|o| o.ty.clone()).collect(),
                abi: function.borrow().clone(),
            }
        } else {
            // This is a normal function.
            Self {
                name,
                ty: AbiEntryTy::Function,
                inputs: function.borrow().inputs.iter().map(|i| i.ty.clone()).collect(),
                outputs: function.borrow().outputs.iter().map(|o| o.ty.clone()).collect(),
                abi: function.borrow().clone(),
            }
        }
    }
}

impl CallableAbiEntry {
    /// Is this entry a state variable?
    pub fn is_state_variable(&self) -> bool {
        matches!(self.ty, AbiEntryTy::StateVariable(_))
    }

    /// Is this entry a function?
    pub fn is_function(&self) -> bool {
        matches!(self.ty, AbiEntryTy::Function)
    }
}

impl fmt::Display for AbiEntryTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "Function"),
            Self::StateVariable(_) => write!(f, "State Variable"),
        }
    }
}

impl fmt::Display for CallableAbiEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:16} {}({})", format!("[{}]", self.ty), self.name, self.inputs.join(", "),)?;

        if self.outputs.len() == 1 {
            write!(f, " return {}", self.outputs[0])
        } else if self.outputs.len() > 1 {
            write!(f, " return ({})", self.outputs.join(", "))
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for ContractTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::Proxy => write!(f, "Proxy"),
            Self::Implementation => write!(f, "Implementation"),
        }
    }
}

impl fmt::Display for CallableAbiInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Contract: {} ({})", self.address, self.contract_ty)?;
        writeln!(f, "Callable ABI Entries ({} entries):", self.entries.len())?;
        for entry in &self.entries {
            writeln!(f, "  {entry}")?;
        }
        Ok(())
    }
}

/// Parse the callable ABI information from a contract.
pub fn parse_callable_abi_info<T>(
    addr: Address,
    contract: T,
    contract_ty: ContractTy,
) -> CallableAbiInfo
where
    T: Borrow<Contract>,
{
    CallableAbiInfo { address: addr, contract_ty, entries: parse_callable_abi_entries(contract) }
}

/// Parse callable ABI entries from a contract
pub fn parse_callable_abi_entries<T>(contract: T) -> Vec<CallableAbiEntry>
where
    T: Borrow<Contract>,
{
    let mut entries: Vec<CallableAbiEntry> = contract
        .borrow()
        .abi
        .as_ref()
        .map(|json_abi| json_abi.functions().map(CallableAbiEntry::from).collect())
        .unwrap_or_default();

    // Sort the entries by type (state variables first), then by name
    entries.sort_by(|a, b| match (&a.ty, &b.ty) {
        (AbiEntryTy::StateVariable(_), AbiEntryTy::Function) => std::cmp::Ordering::Greater,
        (AbiEntryTy::Function, AbiEntryTy::StateVariable(_)) => std::cmp::Ordering::Less,
        _ => a.name.cmp(&b.name),
    });

    entries
}
