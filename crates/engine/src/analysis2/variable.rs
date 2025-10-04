use serde::{Deserialize, Serialize};

use crate::{
    analysis::{UTID, UVID},
    analysis2::{
        ContractBox, FunctionBox, IScope, IType, IVariable, Mutability, ScopeBox, StorageLocation,
        TypeBox, VariableBox,
    },
    ast::{SourceRange, TypeDef, VarDecl},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub uvid: UVID,
    pub decl: VarDecl,
    pub ty: TypeBox,
    pub scope: ScopeBox,
    pub func: Option<FunctionBox>,
    pub contract: Option<ContractBox>,
}

#[typetag::serde]
impl IVariable for Variable {
    fn id(&self) -> UVID {
        self.uvid
    }

    fn unnamed(&self) -> bool {
        self.decl.name.is_empty()
    }

    fn name(&self) -> String {
        self.decl.name.clone()
    }

    fn is_param(&self) -> bool {
        self.decl.param
    }

    fn is_return(&self) -> bool {
        self.decl.ret
    }

    fn is_state_variable(&self) -> bool {
        self.decl.state
    }

    fn declaration_src(&self) -> SourceRange {
        self.decl.src
    }

    fn scope(&self) -> &ScopeBox {
        &self.scope
    }

    fn storage_location(&self) -> &StorageLocation {
        &self.decl.storage
    }

    fn mutability(&self) -> &Mutability {
        &self.decl.mutability
    }

    fn variable_type(&self) -> &TypeBox {
        &self.ty
    }

    fn function(&self) -> Option<&FunctionBox> {
        self.func.as_ref()
    }

    fn contract(&self) -> Option<&ContractBox> {
        self.contract.as_ref()
    }
}
