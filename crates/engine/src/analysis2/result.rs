use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::analysis::UCID;
use crate::analysis::UFID;
use crate::analysis::UTID;
use crate::analysis::UVID;
use crate::analysis2::FunctionBox;
use crate::analysis2::TypeBox;
use crate::analysis2::VariableBox;
use crate::analysis2::{ContractBox, ScopeBox, StepBox};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAnalysis {
    /// Unique identifier for this source file
    pub id: u32,
    /// File system path to the source file
    pub path: PathBuf,
    /// Global variable scope of the source file
    pub global_scope: ScopeBox,
    /// List of analyzed execution steps in this file
    pub steps: Vec<StepBox>,
    /// State variables that should be made public
    pub private_state_variables: Vec<VariableBox>,
    /// List of all contracts in this file.
    pub contracts: HashMap<UCID, ContractBox>,
    /// List of all functions in this file.
    pub functions: HashMap<UFID, FunctionBox>,
    /// List of all state variables in this file.
    pub state_variables: HashMap<UVID, VariableBox>,
    /// Types used and defined in this file.
    pub types: HashMap<UTID, TypeBox>,
    /// Functions that should be made public
    pub private_functions: Vec<FunctionBox>,
    /// Functions that should be made mutable (i.e., neither pure nor view)
    pub immutable_functions: Vec<FunctionBox>,
}
