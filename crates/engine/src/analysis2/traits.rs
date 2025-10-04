use dyn_clone::{clone_trait_object, DynClone};
use serde::{Deserialize, Serialize};

use super::*;
use crate::{
    analysis::{UCID, UFID, USID, UTID, UVID},
    ast::{Func, IfStmt, LoopStmt, SourceRange, Stmt, TryStmt},
};
use std::fmt::Debug;

/// Variable trait that abstracts the common methods of all variables and hides implementation details.
///
/// This trait provides a uniform interface for different variable types (plain, member access,
/// array index, etc.) allowing for better testability and flexibility in the analysis system.
#[typetag::serde(tag = "type")]
pub trait IVariable: Debug + DynClone {
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

    /// Returns the source range of the declaration of this variable.
    fn declaration_src(&self) -> SourceRange;

    /// Returns the scope that this variable is declared in.
    fn scope(&self) -> &ScopeBox;

    /// Returns the storage location of this variable.
    fn storage_location(&self) -> &StorageLocation;

    /// Returns the mutability of this variable.
    fn mutability(&self) -> &Mutability;

    /// Returns the type of the variable.
    fn variable_type(&self) -> &TypeBox;

    /// Returns the function that this variable is declared in, if any.
    ///
    /// State variables and global variables may not have an associated function.
    /// Local variables will have the function they are declared within.
    fn function(&self) -> Option<&FunctionBox>;

    /// Returns the contract that this variable is declared in, if any.
    ///
    /// Variables declared in a free function may not have an associated contract.
    fn contract(&self) -> Option<&ContractBox>;
}
clone_trait_object!(IVariable);
pub type VariableBox = Box<dyn IVariable>;

/// Type trait that abstracts the common methods of Solidity types and hides implementation details.
#[typetag::serde(tag = "type")]
pub trait IType: Debug + DynClone {
    /// The unique type identifier.
    fn id(&self) -> UTID;

    /// Returns the AST node ID of this type.
    fn ast_id(&self) -> usize;
}
clone_trait_object!(IType);
pub type TypeBox = Box<dyn IType>;

/// Scope trait that abstracts the common methods of scopes and hides implementation details.
#[typetag::serde(tag = "type")]
pub trait IScope: Debug + DynClone {
    /// Returns the AST node ID of this scope.
    fn ast_id(&self) -> usize;

    /// Returns the source range of this scope.
    fn src(&self) -> SourceRange;

    /// Returns the parent scope of this scope. A root scope will return None.
    fn parent(&self) -> Option<&ScopeBox>;

    /// Returns the children scopes of this scope.
    fn children(&self) -> Vec<&ScopeBox>;

    /// Adds a child scope to this scope.
    fn add_child(&mut self, child: ScopeBox);

    /// Returns all variables declared in this scope.
    fn declared_variables(&self) -> Vec<VariableBox>;

    /// Returns all variables accessible in this scope, including those declared in parent scopes.
    fn accessible_variables(&self) -> Vec<VariableBox> {
        let mut variables = self.declared_variables();
        if let Some(parent) = self.parent() {
            variables.extend(parent.accessible_variables());
        }
        variables
    }
}
clone_trait_object!(IScope);
pub type ScopeBox = Box<dyn IScope>;

/// Step trait that abstracts the common methods of steps and hides implementation details.
#[typetag::serde(tag = "type")]
pub trait IStep: Debug + DynClone {
    /// The unique step identifier.
    fn id(&self) -> USID;

    /// The variant of the step.
    fn kind(&self) -> &StepKind;

    /// Returns the source range of this step.
    fn src(&self) -> SourceRange;

    /// Returns the variables updated in this step.
    fn updated_variables(&self) -> Vec<VariableBox>;

    /// Returns the number of function calls in this step.
    fn function_call_count(&self) -> usize;

    /// Returns the function that this step belongs to.
    fn function(&self) -> FunctionBox;

    /// Returns the contract that this step belongs to.
    ///
    /// A step may not belong to any contract if it is a step in a free function.
    fn contract(&self) -> Option<ContractBox> {
        self.function().contract()
    }
}
clone_trait_object!(IStep);
pub type StepBox = Box<dyn IStep>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepKind {
    // Function entry
    FuncEntry(Func),
    // Normal statements
    Stmt(Stmt),
    // If condition statement
    If(IfStmt),
    // For/While/DoWhile loop
    Loop(LoopStmt),
    // Try statement
    Try(TryStmt),
}

/// Function trait that abstracts the common methods of functions and hides implementation details.
#[typetag::serde(tag = "type")]
pub trait IFunction: Debug + DynClone {
    /// The unique function identifier.
    fn id(&self) -> UFID;

    /// Returns the contract that this function belongs to.
    ///
    /// A function may not belong to any contract if it is a free function.
    fn contract(&self) -> Option<ContractBox>;
}
clone_trait_object!(IFunction);
pub type FunctionBox = Box<dyn IFunction>;

/// Contract trait that abstracts the common methods of contracts and hides implementation details.
#[typetag::serde(tag = "type")]
pub trait IContract: Debug + DynClone {
    /// The unique contract identifier.
    fn id(&self) -> UCID;

    /// Returns the kind of this contract, i.e., contract, interface, or library.
    fn kind(&self) -> ContractKind;
}
clone_trait_object!(IContract);
pub type ContractBox = Box<dyn IContract>;

/// All Solidity contract kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContractKind {
    /// A normal contract.
    Contract,
    /// An interface.
    Interface,
    /// A library.
    Library,
}

/// A trait for types whose internal data can be cached.
#[typetag::serde(tag = "type")]
pub trait Cacheable: Debug + DynClone {
    /// Clears the cached data.
    fn clear_cache(&mut self);
}
clone_trait_object!(Cacheable);
pub type CacheableBox = Box<dyn Cacheable>;
