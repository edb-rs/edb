use foundry_compilers::artifacts::{Mutability, StorageLocation, Visibility};
use serde::{Deserialize, Serialize};

use super::{SourceRange, TypeDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarDecl {
    pub ast_id: usize,
    pub src: SourceRange,
    pub name: String,
    pub ty: TypeDef,
    pub mutability: Mutability,
    pub storage: StorageLocation,
    pub visibility: Visibility,
    pub state: bool,
    pub param: bool,
    pub ret: bool,
}
