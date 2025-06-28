use crate::{SourceStep, StateSnapshot, UVID};

/// Contains all needed information for debugger.
#[non_exhaustive]
pub struct DebugArtifact {
    pub steps: Vec<DebugStep>,
}

/// Represents the artifact specific to a single debug step.
#[non_exhaustive]
pub struct DebugStep {
    /// The location of this step in the source code.
    pub source_step: SourceStep,

    /// The variables (represented by their UVIDs) that are in scope and accessible at this step.
    pub variables: Vec<UVID>,

    /// The pre-execution state snapshot before executing the current step.
    pub pre_state: StateSnapshot,
}
