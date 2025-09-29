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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_json_abi::{Param, StateMutability};
    use alloy_primitives::address;

    fn create_mock_function(name: &str, inputs: Vec<&str>, outputs: Vec<&str>) -> Function {
        Function {
            name: name.to_string(),
            inputs: inputs
                .into_iter()
                .enumerate()
                .map(|(i, ty)| Param {
                    name: format!("param{i}"),
                    ty: ty.to_string(),
                    internal_type: None,
                    components: Vec::new(),
                })
                .collect(),
            outputs: outputs
                .into_iter()
                .enumerate()
                .map(|(i, ty)| Param {
                    name: format!("output{i}"),
                    ty: ty.to_string(),
                    internal_type: None,
                    components: Vec::new(),
                })
                .collect(),
            state_mutability: StateMutability::View,
        }
    }

    #[test]
    fn test_abi_entry_ty_serialization() {
        let function_ty = AbiEntryTy::Function;
        let state_var_ty = AbiEntryTy::StateVariable(123);

        let function_json =
            serde_json::to_string(&function_ty).expect("Failed to serialize Function");
        let state_var_json =
            serde_json::to_string(&state_var_ty).expect("Failed to serialize StateVariable");

        let function_deserialized: AbiEntryTy =
            serde_json::from_str(&function_json).expect("Failed to deserialize Function");
        let state_var_deserialized: AbiEntryTy =
            serde_json::from_str(&state_var_json).expect("Failed to deserialize StateVariable");

        assert_eq!(function_deserialized, AbiEntryTy::Function);
        assert_eq!(state_var_deserialized, AbiEntryTy::StateVariable(123));
    }

    #[test]
    fn test_contract_ty_serialization() {
        let types = vec![ContractTy::Normal, ContractTy::Proxy, ContractTy::Implementation];

        for contract_ty in types {
            let json = serde_json::to_string(&contract_ty).expect("Failed to serialize ContractTy");
            let deserialized: ContractTy =
                serde_json::from_str(&json).expect("Failed to deserialize ContractTy");
            assert_eq!(deserialized, contract_ty);
        }
    }

    #[test]
    fn test_callable_abi_entry_serialization() {
        let function = create_mock_function("transfer", vec!["address", "uint256"], vec!["bool"]);
        let entry = CallableAbiEntry::from(&function);

        let json = serde_json::to_string(&entry).expect("Failed to serialize CallableAbiEntry");
        let deserialized: CallableAbiEntry =
            serde_json::from_str(&json).expect("Failed to deserialize CallableAbiEntry");

        assert_eq!(deserialized.name, "transfer");
        assert_eq!(deserialized.ty, AbiEntryTy::Function);
        assert_eq!(deserialized.inputs, vec!["address", "uint256"]);
        assert_eq!(deserialized.outputs, vec!["bool"]);
        assert_eq!(deserialized.abi.name, function.name);
    }

    #[test]
    fn test_callable_abi_info_serialization() {
        let address = address!("0x1234567890123456789012345678901234567890");
        let function = create_mock_function("balanceOf", vec!["address"], vec!["uint256"]);
        let entry = CallableAbiEntry::from(&function);

        let info = CallableAbiInfo {
            address,
            contract_ty: ContractTy::Proxy,
            entries: vec![entry.clone()],
        };

        let json = serde_json::to_string(&info).expect("Failed to serialize CallableAbiInfo");
        let deserialized: CallableAbiInfo =
            serde_json::from_str(&json).expect("Failed to deserialize CallableAbiInfo");

        assert_eq!(deserialized.address, address);
        assert_eq!(deserialized.contract_ty, ContractTy::Proxy);
        assert_eq!(deserialized.entries.len(), 1);
        assert_eq!(deserialized.entries[0], entry);
    }

    #[test]
    fn test_callable_abi_entry_from_function() {
        let function = create_mock_function("getValue", vec!["uint256"], vec!["string"]);
        let entry = CallableAbiEntry::from(&function);

        assert_eq!(entry.name, "getValue");
        assert_eq!(entry.ty, AbiEntryTy::Function);
        assert_eq!(entry.inputs, vec!["uint256"]);
        assert_eq!(entry.outputs, vec!["string"]);
        assert!(entry.is_function());
        assert!(!entry.is_state_variable());
    }

    #[test]
    fn test_callable_abi_entry_from_state_variable() {
        let state_var_name = format!("balance{}_edb_state_var_42", "");
        let function = create_mock_function(&state_var_name, vec![], vec!["uint256"]);
        let entry = CallableAbiEntry::from(&function);

        assert_eq!(entry.name, "balance");
        assert_eq!(entry.ty, AbiEntryTy::StateVariable(42));
        assert_eq!(entry.inputs, Vec::<String>::new());
        assert_eq!(entry.outputs, vec!["uint256"]);
        assert!(!entry.is_function());
        assert!(entry.is_state_variable());
    }

    #[test]
    fn test_state_variable_parsing_error() {
        let invalid_name = format!("balance{}_edb_state_var_invalid", "");
        let function = create_mock_function(&invalid_name, vec![], vec!["uint256"]);
        let entry = CallableAbiEntry::from(&function);

        assert_eq!(entry.name, "balance");
        assert_eq!(entry.ty, AbiEntryTy::StateVariable(0)); // Falls back to 0 on parse error
    }

    #[test]
    fn test_abi_entry_ty_display() {
        assert_eq!(format!("{}", AbiEntryTy::Function), "Function");
        assert_eq!(format!("{}", AbiEntryTy::StateVariable(123)), "State Variable");
    }

    #[test]
    fn test_contract_ty_display() {
        assert_eq!(format!("{}", ContractTy::Normal), "Normal");
        assert_eq!(format!("{}", ContractTy::Proxy), "Proxy");
        assert_eq!(format!("{}", ContractTy::Implementation), "Implementation");
    }

    #[test]
    fn test_callable_abi_entry_display() {
        let function = create_mock_function("transfer", vec!["address", "uint256"], vec!["bool"]);
        let entry = CallableAbiEntry::from(&function);

        let display = format!("{entry}");
        assert!(display.contains("[Function]"));
        assert!(display.contains("transfer"));
        assert!(display.contains("address, uint256"));
        assert!(display.contains("return bool"));
    }

    #[test]
    fn test_callable_abi_entry_display_no_outputs() {
        let function = create_mock_function("burn", vec!["uint256"], vec![]);
        let entry = CallableAbiEntry::from(&function);

        let display = format!("{entry}");
        assert!(display.contains("[Function]"));
        assert!(display.contains("burn"));
        assert!(display.contains("uint256"));
        assert!(!display.contains("return"));
    }

    #[test]
    fn test_callable_abi_entry_display_multiple_outputs() {
        let function = create_mock_function("getInfo", vec![], vec!["string", "uint256", "bool"]);
        let entry = CallableAbiEntry::from(&function);

        let display = format!("{entry}");
        assert!(display.contains("[Function]"));
        assert!(display.contains("getInfo"));
        assert!(display.contains("return (string, uint256, bool)"));
    }

    #[test]
    fn test_callable_abi_info_display() {
        let address = address!("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd");
        let function = create_mock_function("test", vec![], vec![]);
        let entry = CallableAbiEntry::from(&function);

        let info = CallableAbiInfo {
            address,
            contract_ty: ContractTy::Implementation,
            entries: vec![entry],
        };

        let display = format!("{info}");
        assert!(display.contains(&format!("{address}")));
        assert!(display.contains("Implementation"));
        assert!(display.contains("1 entries"));
        assert!(display.contains("test"));
    }

    #[test]
    fn test_contract_ty_ordering() {
        let mut types = vec![ContractTy::Implementation, ContractTy::Normal, ContractTy::Proxy];
        types.sort();
        assert_eq!(types, vec![ContractTy::Normal, ContractTy::Proxy, ContractTy::Implementation]);
    }

    #[test]
    fn test_callable_abi_entry_equality() {
        let function1 = create_mock_function("test", vec!["uint256"], vec!["bool"]);
        let function2 = create_mock_function("test", vec!["uint256"], vec!["bool"]);
        let function3 = create_mock_function("test2", vec!["uint256"], vec!["bool"]);

        let entry1 = CallableAbiEntry::from(&function1);
        let entry2 = CallableAbiEntry::from(&function2);
        let entry3 = CallableAbiEntry::from(&function3);

        assert_eq!(entry1, entry2);
        assert_ne!(entry1, entry3);
    }

    #[test]
    fn test_callable_abi_entry_hash() {
        use std::collections::HashSet;

        let function1 = create_mock_function("test", vec!["uint256"], vec!["bool"]);
        let function2 = create_mock_function("test", vec!["uint256"], vec!["bool"]);
        let function3 = create_mock_function("test2", vec!["uint256"], vec!["bool"]);

        let entry1 = CallableAbiEntry::from(&function1);
        let entry2 = CallableAbiEntry::from(&function2);
        let entry3 = CallableAbiEntry::from(&function3);

        let mut set = HashSet::new();
        set.insert(entry1);
        set.insert(entry2); // Should not increase size due to equality
        set.insert(entry3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_large_entry_list_serialization() {
        let address = address!("0xffffffffffffffffffffffffffffffffffffffff");
        let mut entries = Vec::new();

        // Create many entries
        for i in 0..1000 {
            let function = create_mock_function(&format!("func{i}"), vec!["uint256"], vec!["bool"]);
            entries.push(CallableAbiEntry::from(&function));
        }

        let info = CallableAbiInfo { address, contract_ty: ContractTy::Normal, entries };

        let json = serde_json::to_string(&info).expect("Failed to serialize large CallableAbiInfo");
        let deserialized: CallableAbiInfo =
            serde_json::from_str(&json).expect("Failed to deserialize large CallableAbiInfo");

        assert_eq!(deserialized.address, address);
        assert_eq!(deserialized.contract_ty, ContractTy::Normal);
        assert_eq!(deserialized.entries.len(), 1000);
        assert_eq!(deserialized.entries[0].name, "func0");
        assert_eq!(deserialized.entries[999].name, "func999");
    }

    #[test]
    fn test_complex_types_serialization() {
        let function = create_mock_function(
            "complexFunction",
            vec!["address[]", "bytes"],
            vec!["tuple(uint256,string)", "bytes32[]"],
        );
        let entry = CallableAbiEntry::from(&function);

        let json = serde_json::to_string(&entry).expect("Failed to serialize complex types");
        let deserialized: CallableAbiEntry =
            serde_json::from_str(&json).expect("Failed to deserialize complex types");

        assert_eq!(deserialized.inputs, vec!["address[]", "bytes"]);
        assert_eq!(deserialized.outputs, vec!["tuple(uint256,string)", "bytes32[]"]);
    }

    #[test]
    fn test_empty_callable_abi_info_serialization() {
        let info = CallableAbiInfo {
            address: Address::ZERO,
            contract_ty: ContractTy::Normal,
            entries: vec![],
        };

        let json = serde_json::to_string(&info).expect("Failed to serialize empty CallableAbiInfo");
        let deserialized: CallableAbiInfo =
            serde_json::from_str(&json).expect("Failed to deserialize empty CallableAbiInfo");

        assert_eq!(deserialized.address, Address::ZERO);
        assert_eq!(deserialized.contract_ty, ContractTy::Normal);
        assert!(deserialized.entries.is_empty());
    }

    #[test]
    fn test_state_variable_id_edge_cases() {
        // Test with max u64
        let name_max = format!("var{}_edb_state_var_{}", "", u64::MAX);
        let function_max = create_mock_function(&name_max, vec![], vec!["uint256"]);
        let entry_max = CallableAbiEntry::from(&function_max);
        assert_eq!(entry_max.ty, AbiEntryTy::StateVariable(u64::MAX));

        // Test with zero
        let name_zero = format!("var{}_edb_state_var_0", "");
        let function_zero = create_mock_function(&name_zero, vec![], vec!["uint256"]);
        let entry_zero = CallableAbiEntry::from(&function_zero);
        assert_eq!(entry_zero.ty, AbiEntryTy::StateVariable(0));
    }
}
