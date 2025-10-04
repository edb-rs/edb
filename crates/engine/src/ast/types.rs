use std::collections::{BTreeMap, BTreeSet};

use foundry_compilers::artifacts::StateMutability;
use revm::primitives::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeDef {
    Elementary { name: String, mutability: StateMutability },
    Array { base: Box<TypeDef> },
    Mapping { key: Box<TypeDef>, value: Box<TypeDef> },
    Function { parameters: Vec<TypeDef>, return_type: Vec<TypeDef> },
    UserDefined(UserDefinedType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserDefinedType {
    Struct { name: String, members: HashMap<String, TypeDef> },
    Enum { name: String, values: Vec<String> },
    Contract { name: String },
    Alias { name: String, target: Box<TypeDef> },
}
