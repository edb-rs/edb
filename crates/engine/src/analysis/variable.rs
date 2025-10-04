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

//! Variable analysis and representation for Ethereum smart contract analysis.
//!
//! This module provides the core data structures and utilities for representing
//! and tracking variables during smart contract analysis. It includes:
//!
//! - **UVID (Universal Variable Identifier)**: A unique identifier system for
//!   tracking variables across different scopes and contexts
//! - **Variable**: The main data structure representing a smart contract variable
//! - **VariableType**: Enumeration of supported Solidity variable types
//! - **VariableScope**: Structure for managing variable scope information
//!
//! The module is designed to work with the broader analysis framework to provide
//! comprehensive variable tracking and type information during contract analysis.

use foundry_compilers::artifacts::{Expression, TypeName, VariableDeclaration};
use serde::{Deserialize, Serialize};

use crate::analysis::{
    macros::{define_ref, universal_id},
    ContractRef, FunctionRef,
};

// use crate::{
//     // Visitor, Walk
// };

/// The slot where the `edb_runtime_values` mapping is stored.
///
/// This constant represents the first 8 bytes of the keccak256 hash of the string
/// "EDB_RUNTIME_VALUE_OFFSET". It serves as the starting point for UVID generation
/// to ensure unique identifier spaces across different analysis contexts.
pub const EDB_RUNTIME_VALUE_OFFSET: u64 = 0x234c6dfc3bf8fed1;

universal_id! {
    /// A Universal Variable Identifier (UVID) is a unique identifier for a variable in a contract.
    ///
    /// UVIDs provide a way to uniquely identify variables across different scopes,
    /// contexts, and analysis passes. They are used internally by the analysis engine
    /// to track variable relationships and dependencies.
    ///
    /// UVID is also the storage slot that a variable should be stored in storage during debugging. UVID starts from `EDB_RUNTIME_VALUE_OFFSET`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use edb::analysis::variable::{UVID, UVID::next};
    ///
    /// let uvid1 = UVID::next();
    /// let uvid2 = UVID::next();
    /// assert_ne!(uvid1, uvid2);
    /// ```
    UVID => EDB_RUNTIME_VALUE_OFFSET
}

define_ref! {
    /// A reference-counted pointer to a Variable.
    ///
    /// This type alias provides shared ownership of Variable instances, allowing
    /// multiple parts of the analysis system to reference the same variable
    /// without copying the data.
    #[allow(unused)]
    VariableRef(Variable) {
        cached_method: {
            declaration: VariableDeclaration,
            type_name: Option<TypeName>,
        }
        delegate: {
            fn id(&self) -> UVID;
            fn contract(&self) -> Option<ContractRef>;
            fn function(&self) -> Option<FunctionRef>;
        }
    }
}

#[allow(unused)]
impl VariableRef {
    /// Returns the base variable of this variable.
    pub fn base(&self) -> Self {
        let inner = self.inner.read();
        if let Some(base) = inner.base() {
            base
        } else {
            self.clone()
        }
    }
}

/// Represents a variable in a smart contract with its metadata and type information.
///
/// Currently, only local variables are supported.
///
/// The Variable struct contains all the information needed to track and analyze
/// a variable during contract analysis, including its unique identifier, name,
/// declaration details, type, and scope information.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::{Variable, UVID, VariableType, VariableScope};
/// use foundry_compilers::artifacts::VariableDeclaration;
///
/// let variable = Variable {
///     uvid: UVID(1),
///     name: "balance".to_string(),
///     declare: VariableDeclaration::default(),
///     ty: VariableType::Uint(256),
///     scope: VariableScope {},
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
pub enum Variable {
    /// A plain variable with a direct declaration.
    Plain {
        /// The unique variable identifier.
        uvid: UVID,
        /// The variable declaration from the AST.
        declaration: VariableDeclaration,
        /// Whether this is a state variable (true) or local variable (false).
        state_variable: bool,
        /// Function that this variable is declared in.
        function: Option<FunctionRef>,
        /// Contract that this variable is declared in.
        contract: Option<ContractRef>,
    },
    /// A member access variable (e.g., `obj.field`).
    Member {
        /// The base variable being accessed.
        base: VariableRef,
        /// The name of the member being accessed.
        member: String,
    },
    /// An array or mapping index access variable (e.g., `arr[index]`).
    Index {
        /// The base variable being indexed.
        base: VariableRef,
        /// The index expression.
        index: Expression,
    },
    /// An array slice access variable (e.g., `arr[start:end]`).
    IndexRange {
        /// The base variable being sliced.
        base: VariableRef,
        /// The start index expression.
        start: Option<Expression>,
        /// The end index expression.
        end: Option<Expression>,
    },
}

impl Variable {
    /// Returns the unique identifier of this variable.
    pub fn id(&self) -> UVID {
        match self {
            Self::Plain { uvid, .. } => *uvid,
            Self::Member { base, .. } => base.read().id(),
            Self::Index { base, .. } => base.read().id(),
            Self::IndexRange { base, .. } => base.read().id(),
        }
    }

    /// Returns the type name of this variable.
    pub fn type_name(&self) -> Option<TypeName> {
        self.declaration().type_name
    }

    /// Returns the declaration of this variable.
    pub fn declaration(&self) -> VariableDeclaration {
        match self {
            Self::Plain { declaration, .. } => declaration.clone(),
            Self::Member { base, .. } => base.read().declaration(),
            Self::Index { base, .. } => base.read().declaration(),
            Self::IndexRange { base, .. } => base.read().declaration(),
        }
    }

    /// Returns the function of this variable.
    pub fn function(&self) -> Option<FunctionRef> {
        match self {
            Self::Plain { function, .. } => function.clone(),
            Self::Member { base, .. } => base.read().function(),
            Self::Index { base, .. } => base.read().function(),
            Self::IndexRange { base, .. } => base.read().function(),
        }
    }

    /// Returns the contract of this variable.
    pub fn contract(&self) -> Option<ContractRef> {
        match self {
            Self::Plain { contract, .. } => contract.clone(),
            Self::Member { base, .. } => base.read().contract(),
            Self::Index { base, .. } => base.read().contract(),
            Self::IndexRange { base, .. } => base.read().contract(),
        }
    }

    /// Returns the base variable of this variable.
    pub fn base(&self) -> Option<VariableRef> {
        match self {
            Self::Plain { .. } => None,
            Self::Member { base, .. }
            | Self::Index { base, .. }
            | Self::IndexRange { base, .. } => {
                if let Some(base) = base.read().base() {
                    Some(base)
                } else {
                    Some(base.clone())
                }
            }
        }
    }
}
