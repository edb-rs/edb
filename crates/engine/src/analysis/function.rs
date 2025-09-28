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

use foundry_compilers::artifacts::{
    ast::SourceLocation, FunctionDefinition, FunctionTypeName, ModifierDefinition, StateMutability,
    Visibility,
};
use once_cell::sync::OnceCell;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};

use crate::analysis::{macros::universal_id, ContractRef, IFunction, StepRef};

universal_id! {
    /// A Universal Function Identifier (UFID) is a unique identifier for a function in contract execution.
    UFID => 0
}

/// A reference-counted pointer to a Function for efficient sharing across multiple contexts.
///
/// This type alias provides thread-safe reference counting for Function instances,
/// allowing them to be shared between different parts of the analysis system
/// without copying the entire function data.
#[derive(Debug, Clone)]
pub struct FunctionRef {
    inner: Arc<RwLock<Function>>,
    /* cached readonly fields*/
    ufid: OnceCell<UFID>,
    contract: OnceCell<Option<ContractRef>>,
}

impl From<Function> for FunctionRef {
    fn from(function: Function) -> Self {
        Self::new(function)
    }
}

impl FunctionRef {
    /// Creates a new FunctionRef from a Function.
    pub fn new(inner: Function) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
            ufid: OnceCell::new(),
            contract: OnceCell::new(),
        }
    }
}

impl FunctionRef {
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, Function> {
        self.inner.read()
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, Function> {
        self.inner.write()
    }
}

impl FunctionRef {
    /// Returns the UFID of this function.
    pub fn ufid(&self) -> UFID {
        *self.ufid.get_or_init(|| self.inner.read().ufid)
    }

    /// Returns the contract that this function belongs to.
    pub fn contract(&self) -> Option<ContractRef> {
        self.contract.get_or_init(|| self.inner.read().contract.clone()).clone()
    }

    /// Returns the name of this function.
    pub fn name(&self) -> String {
        self.read().definition.name().to_string()
    }

    /// Returns the visibility of this function.
    pub fn visibility(&self) -> Visibility {
        self.read().definition.visibility().clone()
    }

    /// Returns the state mutability of this function.
    pub fn state_mutability(&self) -> Option<StateMutability> {
        self.read().definition.state_mutability().cloned()
    }

    /// Returns the source location of this function.
    pub fn src(&self) -> SourceLocation {
        *self.read().definition.src()
    }
}

impl Serialize for FunctionRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.read().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FunctionRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let function = Function::deserialize(deserializer)?;
        Ok(Self::new(function))
    }
}

/// A reference-counted pointer to a FunctionTypeName for efficient sharing across multiple contexts.
///
/// This type alias provides thread-safe reference counting for FunctionTypeName instances,
/// allowing them to be shared between different parts of the analysis system
/// without copying the entire function data.
#[derive(Debug, Clone)]
pub struct FunctionTypeNameRef {
    inner: Arc<RwLock<FunctionTypeName>>,
}

impl From<FunctionTypeName> for FunctionTypeNameRef {
    fn from(function_type: FunctionTypeName) -> Self {
        Self::new(function_type)
    }
}

impl FunctionTypeNameRef {
    /// Creates a new FunctionTypeNameRef from a FunctionTypeName.
    pub fn new(inner: FunctionTypeName) -> Self {
        Self { inner: Arc::new(RwLock::new(inner)) }
    }
}

#[allow(unused)]
impl FunctionTypeNameRef {
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, FunctionTypeName> {
        self.inner.read()
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, FunctionTypeName> {
        self.inner.write()
    }
}

impl FunctionTypeNameRef {
    /// Returns the visibility of this function type.
    pub fn visibility(&self) -> Visibility {
        self.read().visibility.clone()
    }

    /// Returns the state mutability of this function type.
    pub fn state_mutability(&self) -> StateMutability {
        self.read().state_mutability.clone()
    }

    /// Returns the source location of this function type.
    pub fn src(&self) -> SourceLocation {
        self.read().src
    }
}

impl Serialize for FunctionTypeNameRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.read().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FunctionTypeNameRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let function_type = FunctionTypeName::deserialize(deserializer)?;
        Ok(Self::new(function_type))
    }
}

impl IFunction for FunctionRef {
    type Contract = ContractRef;

    fn id(&self) -> UFID {
        todo!()
    }

    fn contract(&self) -> Option<Self::Contract> {
        todo!()
    }
}
/// Represents a function or modifier in a smart contract with its metadata and type information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// The unique function identifier.
    pub ufid: UFID,
    /// The contract that this function belongs to.
    pub contract: Option<ContractRef>,
    /// The function or modifier definition.
    pub definition: FunctionVariant,
    /// List of steps in this function.
    pub steps: Vec<StepRef>,
}

/// The variant types for function definitions.
#[derive(Debug, Clone, Serialize, Deserialize, derive_more::From)]
pub enum FunctionVariant {
    /// A function definition.
    Function(#[from] FunctionDefinition),
    /// A modifier definition.
    Modifier(#[from] ModifierDefinition),
}

impl FunctionVariant {
    /// Returns the name of this function.
    pub fn name(&self) -> &str {
        match self {
            Self::Function(definition) => &definition.name,
            Self::Modifier(definition) => &definition.name,
        }
    }

    /// Returns the visibility of this function.
    pub fn visibility(&self) -> &Visibility {
        match self {
            Self::Function(definition) => &definition.visibility,
            Self::Modifier(definition) => &definition.visibility,
        }
    }

    /// Returns the source location of this function.
    pub fn src(&self) -> &SourceLocation {
        match self {
            Self::Function(definition) => &definition.src,
            Self::Modifier(definition) => &definition.src,
        }
    }

    /// Returns the state mutability of this function.
    pub fn state_mutability(&self) -> Option<&StateMutability> {
        match self {
            Self::Function(definition) => definition.state_mutability.as_ref(),
            Self::Modifier(_) => None,
        }
    }
}

impl Function {
    /// Creates a new Function with the given contract and definition.
    pub fn new_function(contract: Option<ContractRef>, definition: FunctionDefinition) -> Self {
        Self {
            ufid: UFID::next(),
            contract,
            definition: FunctionVariant::Function(definition),
            steps: vec![],
        }
    }

    /// Creates a new Function with the given contract and definition.
    pub fn new_modifier(contract: ContractRef, definition: ModifierDefinition) -> Self {
        Self {
            ufid: UFID::next(),
            contract: Some(contract),
            definition: FunctionVariant::Modifier(definition),
            steps: vec![],
        }
    }
}
