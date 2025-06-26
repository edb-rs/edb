use foundry_compilers::artifacts::ast::SourceLocation;

use crate::StateSnapshot;

/// Contains all needed information for debugger.
pub struct DebugArtifact {
    pub steps: Vec<DebugStep>,
}

/// Represents the artifact specific to a single debug step.
pub struct DebugStep {
    /// The location of this step in
    pub source_location: SourceLocation,

    /// The pre-execution state snapshot before executing the current step.
    pub pre_state: StateSnapshot,
}
