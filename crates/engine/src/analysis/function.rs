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

use foundry_compilers::artifacts::{
    ast::SourceLocation, FunctionDefinition, FunctionTypeName, ModifierDefinition, StateMutability,
    Visibility,
};
use serde::{Deserialize, Serialize};

use crate::analysis::{
    macros::{define_ref, universal_id},
    ContractRef, StepRef,
};

universal_id! {
    /// A Universal Function Identifier (UFID) is a unique identifier for a function in contract execution.
    UFID => 0
}

define_ref! {
    /// A reference-counted pointer to a Function for efficient sharing across multiple contexts.
    ///
    /// This type alias provides thread-safe reference counting for Function instances,
    /// allowing them to be shared between different parts of the analysis system
    /// without copying the entire function data.
    FunctionRef(Function) {
        clone_field: {
            ufid: UFID,
            contract: Option<ContractRef>,
        }
    }
}

impl FunctionRef {
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

define_ref! {
    /// A reference-counted pointer to a FunctionTypeName for efficient sharing across multiple contexts.
    ///
    /// This type alias provides thread-safe reference counting for FunctionTypeName instances,
    /// allowing them to be shared between different parts of the analysis system
    /// without copying the entire function data.
    FunctionTypeNameRef(FunctionTypeName) {
        clone_field: {
            visibility: Visibility,
            state_mutability: StateMutability,
            src: SourceLocation,
        }
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
