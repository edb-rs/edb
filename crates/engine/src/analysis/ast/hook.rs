use serde::{Deserialize, Serialize};

/// Locations of different types of hooks in a [`crate::analysis::Step`]. The locations are the source string index in the same source file as the corresponding step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepHookLocations {
    /// The location to instrument hooks before the step
    pub before_step: usize,
    /// The locations to instrument hooks after the step. The hooks will be instrumented after all the locations in this vector.
    pub after_step: Vec<usize>,
}
