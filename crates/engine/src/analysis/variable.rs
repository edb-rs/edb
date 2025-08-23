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

use std::sync::{Arc, Mutex};

use alloy_primitives::{map::foldhash::HashMap, U256};
use derive_more::From;
use foundry_compilers::artifacts::{
    ast::SourceLocation, Block, ContractDefinition, Expression, ForStatement, FunctionDefinition,
    SourceUnit, UncheckedBlock, VariableDeclaration,
};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

// use crate::{
//     // Visitor, Walk
// };

lazy_static! {
    /// Global counter for generating unique variable identifiers (UVIDs). It is
    /// also the storage slot that a variable should be stored in storage during debugging.
    ///
    /// This mutex-protected counter ensures thread-safe generation of unique
    /// identifiers across multiple analysis contexts. The counter starts from
    /// `EDB_RUNTIME_VALUE_OFFSET` to avoid conflicts with other identifier spaces.
    pub static ref NEXT_UVID: Mutex<UVID> = Mutex::new(UVID(EDB_RUNTIME_VALUE_OFFSET));
}

/// The slot where the `edb_runtime_values` mapping is stored.
///
/// This constant represents the first 8 bytes of the keccak256 hash of the string
/// "EDB_RUNTIME_VALUE_OFFSET". It serves as the starting point for UVID generation
/// to ensure unique identifier spaces across different analysis contexts.
pub const EDB_RUNTIME_VALUE_OFFSET: u64 = 0x234c6dfc3bf8fed1;

/// A Universal Variable Identifier (UVID) is a unique identifier for a variable in a contract.
///
/// UVIDs provide a way to uniquely identify variables across different scopes,
/// contexts, and analysis passes. They are used internally by the analysis engine
/// to track variable relationships and dependencies.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::{UVID, new_uvid};
///
/// let uvid1 = new_uvid();
/// let uvid2 = new_uvid();
/// assert_ne!(uvid1, uvid2);
/// ```
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct UVID(u64);

impl UVID {
    /// Increment the UVID and return the previous value.
    ///
    /// This method atomically increments the internal counter and returns
    /// the previous value, ensuring each call produces a unique identifier.
    ///
    /// # Returns
    ///
    /// The previous UVID value before incrementing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use edb::analysis::variable::UVID;
    ///
    /// let mut uvid = UVID(42);
    /// let previous = uvid.inc();
    /// assert_eq!(previous, UVID(42));
    /// assert_eq!(uvid, UVID(43));
    /// ```
    pub fn inc(&mut self) -> Self {
        let v = *self;
        self.0 += 1;
        v
    }
}

impl From<UVID> for u64 {
    /// Convert a UVID to its underlying u64 representation.
    fn from(uvid: UVID) -> Self {
        uvid.0
    }
}

impl From<UVID> for U256 {
    /// Convert a UVID to a U256 representation for use in Ethereum-related operations.
    fn from(uvid: UVID) -> Self {
        Self::from(uvid.0)
    }
}

/// Generate a new unique variable identifier (UVID).
///
/// This function provides a thread-safe way to generate unique identifiers
/// for variables. Each call returns a new UVID that is guaranteed to be
/// unique within the current analysis session.
///
/// # Returns
///
/// A new unique UVID.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::new_uvid;
///
/// let uvid1 = new_uvid();
/// let uvid2 = new_uvid();
/// assert_ne!(uvid1, uvid2);
/// ```
pub fn new_uvid() -> UVID {
    let mut uvid = NEXT_UVID.lock().unwrap();
    uvid.inc()
}

/// A reference-counted pointer to a Variable.
///
/// This type alias provides shared ownership of Variable instances, allowing
/// multiple parts of the analysis system to reference the same variable
/// without copying the data.
pub type VariableRef = Arc<Variable>;

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
        start: Expression,
        /// The end index expression.
        end: Expression,
    },
}

impl Variable {
    /// Returns a human-readable string representation of the variable.
    ///
    /// This method provides a concise display format for variables:
    /// - Plain variables show their declaration name
    /// - Member access shows `base.member`
    /// - Index access shows `base[.]`
    /// - Index range shows `base[..]`
    pub fn pretty_display(&self) -> String {
        match self {
            Self::Plain { declaration, .. } => declaration.name.clone(),
            Self::Member { base, member } => format!("{}.{}", base.pretty_display(), member),
            Self::Index { base, .. } => format!("{}[.]", base.pretty_display()),
            Self::IndexRange { base, .. } => {
                format!("{}[..]", base.pretty_display())
            }
        }
    }
}

/// A reference-counted pointer to a VariableScope.
pub type VariableScopeRef = Arc<VariableScope>;

/// Represents the scope and visibility information for a variable.
///
/// This structure contains information about where a variable is defined
/// and how it can be accessed. Currently, this is a placeholder structure
/// that can be extended with additional scope-related information as needed.
///
/// # Future Extensions
///
/// This structure may be extended to include:
/// - Function scope information
/// - Contract scope information
/// - Visibility modifiers (public, private, internal, external)
/// - Storage location (storage, memory, calldata)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VariableScope {
    /// The AST node that defines this scope
    pub node: ScopeNode,
    /// Variables declared in this scope, mapped by their UVID
    pub variables: HashMap<UVID, VariableRef>,
    /// Child scopes contained within this scope
    pub children: Vec<VariableScopeRef>,
}

impl VariableScope {
    /// Returns the unique identifier of this scope, i.e., the node ID of the AST node that corresponds to this scope.
    pub fn id(&self) -> usize {
        self.node.id()
    }

    /// Returns the source location of this scope's AST node.
    pub fn src(&self) -> SourceLocation {
        self.node.src()
    }

    /// Returns a human-readable string representation of the scope hierarchy.
    ///
    /// This method displays the scope and all its child scopes in a tree-like format,
    /// showing the variables contained in each scope.
    pub fn pretty_display(&self) -> String {
        self.pretty_display_with_indent(0)
    }

    fn pretty_display_with_indent(&self, indent_level: usize) -> String {
        let mut result = String::new();
        let indent = "  ".repeat(indent_level);

        // Print current scope's variables
        if self.variables.is_empty() {
            result.push_str(&format!("{}Scope({}): {{}}", indent, self.node.variant_name()));
        } else {
            let mut variable_names: Vec<String> =
                self.variables.values().map(|var| var.pretty_display()).collect();
            variable_names.sort(); // Sort for consistent output
            result.push_str(&format!(
                "{}Scope({}): {{{}}}",
                indent,
                self.node.variant_name(),
                variable_names.join(", ")
            ));
        }

        // Print children scopes recursively with increased indentation
        for child in &self.children {
            result.push('\n');
            result.push_str(&child.pretty_display_with_indent(indent_level + 1));
        }

        result
    }
}

/// Represents the type of a smart contract variable.
///
/// This enum covers the basic Solidity types that are commonly used in
/// smart contract analysis. The types are designed to be extensible for
/// future additions.
///
/// # Examples
///
/// ```rust
/// use edb::analysis::variable::VariableType;
///
/// let uint_type = VariableType::Uint(256);
/// let address_type = VariableType::Address;
/// let bool_type = VariableType::Bool;
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VariableType {
    /// A `uint` type variable. The number of bits is specified by the parameter.
    ///
    /// For instance, `Uint(8)` denotes a `uint8` Solidity type, while `Uint(256)`
    /// represents a `uint256` (the default uint type in Solidity).
    Uint(u8),
    /// An `address` type variable representing an Ethereum address.
    ///
    /// This type is used for variables that store 20-byte Ethereum addresses.
    Address,
    /// A `bool` type variable representing a boolean value.
    ///
    /// This type is used for variables that can be either `true` or `false`.
    Bool,
}

/// Represents different types of AST nodes that can define variable scopes.
///
/// This enum wraps various Solidity AST node types that create new variable scopes,
/// allowing the variable analyzer to track scope boundaries and variable visibility.
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum ScopeNode {
    /// A source unit scope (file-level).
    SourceUnit(#[from] SourceUnit),
    /// A block statement scope.
    Block(#[from] Block),
    /// An unchecked block scope.
    UncheckedBlock(#[from] UncheckedBlock),
    /// A for loop scope.
    ForStatement(#[from] ForStatement),
    /// A contract definition scope.
    ContractDefinition(#[from] ContractDefinition),
    /// A function definition scope.
    FunctionDefinition(#[from] FunctionDefinition),
}

impl ScopeNode {
    /// Returns the node ID of the AST node.
    pub fn id(&self) -> usize {
        match self {
            Self::SourceUnit(source_unit) => source_unit.id,
            Self::Block(block) => block.id,
            Self::UncheckedBlock(unchecked_block) => unchecked_block.id,
            Self::ForStatement(for_statement) => for_statement.id,
            Self::ContractDefinition(contract_definition) => contract_definition.id,
            Self::FunctionDefinition(function_definition) => function_definition.id,
        }
    }

    /// Returns the source location of the wrapped AST node.
    pub fn src(&self) -> SourceLocation {
        match self {
            Self::SourceUnit(source_unit) => source_unit.src,
            Self::Block(block) => block.src,
            Self::UncheckedBlock(unchecked_block) => unchecked_block.src,
            Self::ForStatement(for_statement) => for_statement.src,
            Self::ContractDefinition(contract_definition) => contract_definition.src,
            Self::FunctionDefinition(function_definition) => function_definition.src,
        }
    }

    /// Returns a string representation of the scope node variant name.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::SourceUnit(_) => "SourceUnit",
            Self::Block(_) => "Block",
            Self::UncheckedBlock(_) => "UncheckedBlock",
            Self::ForStatement(_) => "ForStatement",
            Self::ContractDefinition(_) => "ContractDefinition",
            Self::FunctionDefinition(_) => "FunctionDefinition",
        }
    }
}
