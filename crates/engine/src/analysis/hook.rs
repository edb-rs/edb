use crate::{SourceStepRef, VariableRef};

/// Contains information that should be recoreded before and after each step.
#[derive(Debug, Clone)]
pub enum StepHook {
    /// The hook to mark that a step is about to be executed. The debugger will pause here during step-by-step execution.
    BeforeStep(SourceStepRef),

    /// The hook to mark which variables becomes in scope.
    VariableInScope(VariableRef),

    /// The hook to mark that a variable becomes out of scope.
    VariableOutOfScope(VariableRef),

    /// The hook to mark that a variable is updated.
    VariableUpdate(VariableRef),
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
