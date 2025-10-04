use serde::{Deserialize, Serialize};

use crate::{analysis::UTID, analysis2::IType, ast::TypeDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Type {
    pub utid: UTID,
    pub ast_id: usize,
    pub ty: TypeDef,
}

#[typetag::serde]
impl IType for Type {
    fn id(&self) -> UTID {
        self.utid
    }

    fn ast_id(&self) -> usize {
        self.ast_id
    }
}
