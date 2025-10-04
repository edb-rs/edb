use serde::{Deserialize, Serialize};

use crate::{
    analysis::USID,
    analysis2::{FunctionBox, IStep, ScopeBox, StepKind, VariableBox},
    ast::SourceRange,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub usid: USID,
    pub kind: StepKind,
    pub src: SourceRange,
    pub scope: ScopeBox,
    pub function_call_count: usize,
    pub func: FunctionBox,
    pub accessible_variables: Vec<VariableBox>,
    pub declared_variables: Vec<VariableBox>,
    pub updated_variables: Vec<VariableBox>,
}

#[typetag::serde]
impl IStep for Step {
    fn id(&self) -> USID {
        self.usid
    }

    fn kind(&self) -> &StepKind {
        &self.kind
    }

    fn src(&self) -> SourceRange {
        self.src
    }

    fn updated_variables(&self) -> Vec<VariableBox> {
        self.updated_variables.clone()
    }

    fn function_call_count(&self) -> usize {
        self.function_call_count
    }

    fn function(&self) -> FunctionBox {
        self.func.clone()
    }
}
