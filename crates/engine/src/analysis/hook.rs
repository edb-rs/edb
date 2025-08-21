use serde::{Deserialize, Serialize};

use crate::{USID, UVID};

/// Contains information that should be recoreded before and after each step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepHook {
    /// The hook to mark that a step is about to be executed. The debugger will pause here during step-by-step execution.
    BeforeStep(USID),

    /// The hook to mark which variables becomes in scope.
    VariableInScope(UVID),

    /// The hook to mark that a variable becomes out of scope.
    VariableOutOfScope(UVID),

    /// The hook to mark that a variable is updated.
    VariableUpdate(UVID),
}

impl StepHook {
    /// Returns the variant name of this hook.
    ///
    /// # Returns
    ///
    /// A string slice representing the variant name.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::BeforeStep(_) => "BeforeStep",
            Self::VariableInScope(_) => "VariableInScope",
            Self::VariableOutOfScope(_) => "VariableOutOfScope",
            Self::VariableUpdate(_) => "VariableUpdate",
        }
    }
}
