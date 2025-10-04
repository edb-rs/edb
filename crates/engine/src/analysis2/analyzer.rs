use std::collections::HashMap;

use crate::{
    analysis::{UTID, UVID},
    analysis2::{
        Contract, ContractBox, Function, FunctionBox, Scope, ScopeBox, Step, StepBox, Type,
        TypeBox, Variable, VariableBox,
    },
};

pub struct Analyzer {
    source_id: u32,

    scope_stack: Vec<Scope>,

    finished_steps: Vec<Step>,
    current_step: Option<Step>,

    functions: Vec<Function>,
    current_function: Option<Function>,

    contracts: Vec<Contract>,
    current_contract: Option<Contract>,

    ast_id_to_variables: HashMap<UVID, Variable>,

    types: Vec<Type>,
}

impl Analyzer {
    pub fn new(source_id: u32) -> Self {
        Self {
            source_id,
            scope_stack: vec![],
            finished_steps: vec![],
            current_step: None,
            functions: vec![],
            current_function: None,
            contracts: vec![],
            current_contract: None,
            ast_id_to_variables: HashMap::new(),
            types: vec![],
        }
    }
}
