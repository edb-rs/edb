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

mod analyzer;
pub use analyzer::*;

pub mod analyzer2;

mod ast;
pub use ast::*;

mod common;
pub use common::*;

mod contract;
pub use contract::*;

mod function;
pub use function::*;

mod step;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
pub use step::*;

mod types;
pub use types::*;

mod variable;
pub use variable::*;

mod annotation;
pub use annotation::*;

mod visitor;
pub use visitor::*;

mod source;
pub use source::*;

// Import necessary types for the trait definitions
use auto_impl::auto_impl;
use foundry_compilers::artifacts::{ast::SourceLocation, TypeName, VariableDeclaration};

mod macros {
    macro_rules! universal_id {
        (
            $(#[$attr:meta])*
            $name:ident => $initial_value:expr
        ) => {
            $(#[$attr])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Serialize, Deserialize)]
            pub struct $name(u64);

            paste::paste! {
                lazy_static::lazy_static! {
                    /// The global counter for the $name.
                    #[doc = "The global counter for the " $name " object."]
                    pub static ref [<NEXT_ $name>]: std::sync::Mutex<$name> = std::sync::Mutex::new($name($initial_value));
                }
            }

            paste::paste! {
                impl $name {
                    /// Get the next value and increment the global counter.
                    pub fn next() -> Self {
                        let mut counter = [<NEXT_ $name>].lock().unwrap();
                        let value = *counter;
                        counter.0 += 1;
                        value
                    }
                }
            }

            impl From<$name> for u64 {
                fn from(value: $name) -> Self {
                    value.0
                }
            }

            impl From<$name> for alloy_primitives::U256 {
                fn from(value: $name) -> Self {
                    Self::from(value.0)
                }
            }

            impl From<u64> for $name {
                fn from(value: u64) -> Self {
                    Self(value)
                }
            }

            impl TryFrom<alloy_primitives::U256> for $name {
                type Error = alloy_primitives::ruint::FromUintError<u64>;
                fn try_from(value: alloy_primitives::U256) -> Result<Self, alloy_primitives::ruint::FromUintError<u64>> {
                    value.try_into().map(Self)
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.0)
                }
            }
        };
    }

    pub(crate) use universal_id;
}

/// A trait collecting all types used in the analysis.
pub trait AnalysisTypes {
    /// The variable type.
    type Variable: IVariable<
        Scope = Self::Scope,
        Function = Self::Function,
        Contract = Self::Contract,
    >;
    /// The type type.
    type UserDefinedType: IUserDefinedType;
    /// The scope type.
    type Scope: IScope<Variable = Self::Variable>;
    /// The step type.
    type Step: IStep<Function = Self::Function, Contract = Self::Contract>;
    /// The function type.
    type Function: IFunction<Contract = Self::Contract>;
    /// The contract type.
    type Contract: IContract;
}

#[derive(Debug, Clone, Copy)]
pub struct EDBAnalysisTypes;

impl AnalysisTypes for EDBAnalysisTypes {
    type Variable = VariableRef;
    type UserDefinedType = UserDefinedTypeRef;
    type Scope = VariableScopeRef;
    type Step = StepRef;
    type Function = FunctionRef;
    type Contract = ContractRef;
}

/// Variable trait that abstracts the common methods of all variables and hides implementation details.
///
/// This trait provides a uniform interface for different variable types (plain, member access,
/// array index, etc.) allowing for better testability and flexibility in the analysis system.
#[auto_impl(Box, &, Arc, Rc)]
pub trait IVariable: Clone + Serialize + DeserializeOwned {
    /// The scope type.
    type Scope: IScope;
    /// The function type.
    type Function: IFunction;
    /// The contract type.
    type Contract: IContract;

    /// Returns the unique identifier of this variable.
    ///
    /// The UVID (Universal Variable Identifier) is used to uniquely identify variables
    /// across different scopes and contexts in the analysis system.
    fn id(&self) -> UVID;

    /// Returns whether this variable is unnamed. A typical unnamed variable is a function return variable.
    fn unnamed(&self) -> bool;

    /// Returns the identifier of the variable, i.e., the variable name. The unnamed variable will return an empty string.
    fn name(&self) -> String;

    /// Returns whether this variable is a function parameter.
    fn is_param(&self) -> bool;

    /// Returns whether this variable is a function return variable.
    fn is_return(&self) -> bool;

    /// Returns whether this is a state variable (true) or local variable (false).
    ///
    /// This is primarily relevant for plain variables. Member access and index
    /// access variables inherit this property from their base variable.
    fn is_state_variable(&self) -> bool;

    /// Returns whether this is a local variable (true) or state variable (false).
    fn is_local_variable(&self) -> bool {
        !self.is_state_variable()
    }

    /// Returns the scope that this variable is declared in.
    fn scope(&self) -> Self::Scope;

    /// Returns the function that this variable is declared in, if any.
    ///
    /// State variables and global variables may not have an associated function.
    /// Local variables will have the function they are declared within.
    fn function(&self) -> Option<Self::Function>;

    /// Returns the contract that this variable is declared in, if any.
    ///
    /// Variables declared in a free function may not have an associated contract.
    fn contract(&self) -> Option<Self::Contract>;
}

/// Type trait that abstracts the common methods of Solidity types and hides implementation details.
#[auto_impl(Box, &, Arc, Rc)]
pub trait IUserDefinedType: Clone + Serialize + DeserializeOwned {
    /// The unique type identifier.
    fn id(&self) -> UTID;

    /// Returns the AST node ID of this type.
    fn ast_id(&self) -> usize;
}

/// Scope trait that abstracts the common methods of scopes and hides implementation details.
pub trait IScope: Clone + Serialize + DeserializeOwned {
    /// The variable type.
    type Variable: IVariable;

    /// Returns the AST node ID of this scope.
    fn ast_id(&self) -> usize;

    /// Returns the source range of this scope.
    fn src(&self) -> SourceRange;

    /// Returns the parent scope of this scope. A root scope will return None.
    fn parent(&self) -> Option<&Self>;

    /// Returns the children scopes of this scope.
    fn children(&self) -> Vec<&Self>;

    /// Adds a child scope to this scope.
    fn add_child(&mut self, child: &Self);

    /// Returns all variables declared in this scope.
    fn declared_variables(&self) -> Vec<Self::Variable>;

    /// Returns all variables accessible in this scope, including those declared in parent scopes.
    fn accessible_variables(&self) -> Vec<Self::Variable> {
        let mut variables = self.declared_variables();
        if let Some(parent) = self.parent() {
            variables.extend(parent.accessible_variables());
        }
        variables
    }
}

/// Step trait that abstracts the common methods of steps and hides implementation details.
#[auto_impl(Box, &, Arc, Rc)]
pub trait IStep: Clone + Serialize + DeserializeOwned {
    /// The function type.
    type Function: IFunction<Contract = Self::Contract>;
    /// The contract type.
    type Contract: IContract;

    /// The unique step identifier.
    fn id(&self) -> USID;

    /// Returns the source range of this step.
    fn src(&self) -> SourceRange;

    /// Returns the function that this step belongs to.
    fn function(&self) -> Self::Function;

    /// Returns the contract that this step belongs to.
    ///
    /// A step may not belong to any contract if it is a step in a free function.
    fn contract(&self) -> Option<Self::Contract> {
        self.function().contract()
    }
}

/// Function trait that abstracts the common methods of functions and hides implementation details.
#[auto_impl(Box, &, Arc, Rc)]
pub trait IFunction: Clone + Serialize + DeserializeOwned {
    /// The contract type.
    type Contract: IContract;

    /// The unique function identifier.
    fn id(&self) -> UFID;

    /// Returns the contract that this function belongs to.
    ///
    /// A function may not belong to any contract if it is a free function.
    fn contract(&self) -> Option<Self::Contract>;
}

/// Contract trait that abstracts the common methods of contracts and hides implementation details.
#[auto_impl(Box, &, Arc, Rc)]
pub trait IContract: Clone + Serialize + DeserializeOwned {
    /// The unique contract identifier.
    fn id(&self) -> UCID;
}

/// A trait for types whose internal data can be cached.
pub trait Cacheable {
    /// Clears the cached data.
    fn clear_cache(&mut self);
}
