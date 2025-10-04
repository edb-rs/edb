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

//! Variable scope analysis and representation.
//!
//! This module provides data structures for managing variable scopes during
//! smart contract analysis, including scope hierarchies and variable visibility.

use derive_more::From;
use foundry_compilers::artifacts::{
    ast::SourceLocation, Block, ContractDefinition, ForStatement, FunctionDefinition,
    ModifierDefinition, SourceUnit, UncheckedBlock,
};
use serde::{Deserialize, Serialize};

use crate::analysis::{macros::define_ref, VariableRef};

define_ref! {
    /// A reference-counted pointer to a VariableScope.
    VariableScopeRef(VariableScope) {
        cached_field: {
            children: Vec<VariableScopeRef>,
            declared_variables: Vec<VariableRef>,
        }
        delegate: {
            /// Returns the node ID of the AST node that corresponds to this scope.
            fn ast_id(&self) -> usize;
            /// Returns the source location of this scope's AST node.
            fn src(&self) -> SourceLocation;
        }
        additional_cache: {
            variables_recursive: Vec<VariableRef>,
        }
    }
}

/* Cached read methods */
impl VariableScopeRef {
    /// Returns all variables in this scope and its parent scopes recursively. The variables are cached.
    pub fn variables_recursive(&self) -> &Vec<VariableRef> {
        self.variables_recursive.get_or_init(|| {
            let mut variables = self.read().declared_variables.clone();
            variables.extend(
                self.inner
                    .read()
                    .parent
                    .as_ref()
                    .map_or(vec![], |parent| parent.variables_recursive().clone()),
            );
            variables
        })
    }
}

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
#[derive(Clone, Serialize, Deserialize, derive_more::Debug)]
#[non_exhaustive]
pub struct VariableScope {
    /// The AST node that defines this scope
    pub node: ScopeNode,
    /// Variables declared in this scope, mapped by their UVID
    pub declared_variables: Vec<VariableRef>,
    /// Parent scope
    pub parent: Option<VariableScopeRef>,
    /// Child scopes contained within this scope
    pub children: Vec<VariableScopeRef>,
}

impl VariableScope {
    /// Returns the unique identifier of this scope, i.e., the node ID of the AST node that corresponds to this scope.
    pub fn ast_id(&self) -> usize {
        self.node.ast_id()
    }

    /// Returns the source location of this scope's AST node.
    pub fn src(&self) -> SourceLocation {
        self.node.src()
    }

    /// Returns all variables in this scope and its parent scopes recursively. The variables are not cached.
    pub fn variables_recursive(&self) -> Vec<VariableRef> {
        let mut variables = self.declared_variables.clone();
        variables.extend(
            self.parent.clone().map_or(vec![], |parent| parent.read().variables_recursive()),
        );
        variables
    }
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
    /// A modifier definition scope.
    ModifierDefinition(#[from] ModifierDefinition),
}

impl ScopeNode {
    /// Returns the node ID of the AST node.
    pub fn ast_id(&self) -> usize {
        match self {
            Self::SourceUnit(source_unit) => source_unit.id,
            Self::Block(block) => block.id,
            Self::UncheckedBlock(unchecked_block) => unchecked_block.id,
            Self::ForStatement(for_statement) => for_statement.id,
            Self::ContractDefinition(contract_definition) => contract_definition.id,
            Self::FunctionDefinition(function_definition) => function_definition.id,
            Self::ModifierDefinition(modifier_definition) => modifier_definition.id,
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
            Self::ModifierDefinition(modifier_definition) => modifier_definition.src,
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
            Self::ModifierDefinition(_) => "ModifierDefinition",
        }
    }
}
