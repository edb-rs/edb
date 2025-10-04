use serde::{Deserialize, Serialize};

use crate::{
    analysis::UCID,
    analysis2::{ContractKind, IContract},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub ucid: UCID,
    pub kind: ContractKind,
}

#[typetag::serde]
impl IContract for Contract {
    fn id(&self) -> UCID {
        self.ucid
    }

    fn kind(&self) -> ContractKind {
        self.kind
    }
}
