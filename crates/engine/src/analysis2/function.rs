use serde::{Deserialize, Serialize};

use crate::{
    analysis::UFID,
    analysis2::{ContractBox, IFunction, StepBox},
    ast::Func,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub ufid: UFID,
    pub steps: Vec<StepBox>,
    pub definition: Func,
    pub contract: Option<ContractBox>,
}

#[typetag::serde]
impl IFunction for Function {
    fn id(&self) -> UFID {
        self.ufid
    }

    fn contract(&self) -> Option<ContractBox> {
        self.contract.clone()
    }
}
