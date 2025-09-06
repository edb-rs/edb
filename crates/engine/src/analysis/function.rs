use std::sync::Arc;

use foundry_compilers::artifacts::{
    ast::SourceLocation, FunctionDefinition, FunctionTypeName, ModifierDefinition, StateMutability,
    Visibility,
};
use once_cell::sync::OnceCell;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};

use crate::analysis::{macros::universal_id, ContractRef, StepRef};

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
}

impl From<Function> for FunctionRef {
    fn from(function: Function) -> Self {
        Self::new(function)
    }
}

impl FunctionRef {
    /// Creates a new FunctionRef from a Function.
    pub fn new(inner: Function) -> Self {
        Self { inner: Arc::new(RwLock::new(inner)), ufid: OnceCell::new() }
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
    pub fn ufid(&self) -> UFID {
        *self.ufid.get_or_init(|| self.inner.read().ufid)
    }

    pub fn name(&self) -> String {
        self.read().definition.name().to_string()
    }

    pub fn visibility(&self) -> Visibility {
        self.read().definition.visibility().clone()
    }

    pub fn state_mutability(&self) -> Option<StateMutability> {
        self.read().definition.state_mutability().cloned()
    }

    pub fn src(&self) -> SourceLocation {
        self.read().definition.src().clone()
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

impl FunctionTypeNameRef {
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, FunctionTypeName> {
        self.inner.read()
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, FunctionTypeName> {
        self.inner.write()
    }
}

impl FunctionTypeNameRef {
    pub fn visibility(&self) -> Visibility {
        self.read().visibility.clone()
    }

    pub fn state_mutability(&self) -> StateMutability {
        self.read().state_mutability.clone()
    }

    pub fn src(&self) -> SourceLocation {
        self.read().src.clone()
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

#[derive(Debug, Clone, Serialize, Deserialize, derive_more::From)]
pub enum FunctionVariant {
    Function(#[from] FunctionDefinition),
    Modifier(#[from] ModifierDefinition),
}

impl FunctionVariant {
    pub fn name(&self) -> &str {
        match self {
            FunctionVariant::Function(definition) => &definition.name,
            FunctionVariant::Modifier(definition) => &definition.name,
        }
    }

    pub fn visibility(&self) -> &Visibility {
        match self {
            FunctionVariant::Function(definition) => &definition.visibility,
            FunctionVariant::Modifier(definition) => &definition.visibility,
        }
    }

    pub fn src(&self) -> &SourceLocation {
        match self {
            FunctionVariant::Function(definition) => &definition.src,
            FunctionVariant::Modifier(definition) => &definition.src,
        }
    }

    pub fn state_mutability(&self) -> Option<&StateMutability> {
        match self {
            FunctionVariant::Function(definition) => definition.state_mutability.as_ref(),
            FunctionVariant::Modifier(_) => None,
        }
    }
}

impl Function {
    pub fn new_function(contract: Option<ContractRef>, definition: FunctionDefinition) -> Self {
        Self {
            ufid: UFID::next(),
            contract,
            definition: FunctionVariant::Function(definition),
            steps: vec![],
        }
    }

    pub fn new_modifier(contract: ContractRef, definition: ModifierDefinition) -> Self {
        Self {
            ufid: UFID::next(),
            contract: Some(contract),
            definition: FunctionVariant::Modifier(definition),
            steps: vec![],
        }
    }
}
