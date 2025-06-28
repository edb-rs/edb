use foundry_compilers::artifacts::Statement;

use crate::UnitLocation;

/// A set of source code statements or lines that are executed in a single debugger step.
pub enum SourceStep {
    /// A consecutive list of statements that are executed in a single debug step.
    Statements(Vec<Statement>, UnitLocation),
    /// A consecutive list of lines that are executed in a single debug step. Although `Statements`
    /// should be preferred to use whenever possible, some corner cases may exist such that a
    /// single debug step is not one or multiple statements, e.g., the step of executing the line
    /// of `for(...;...;...)` in a loop.
    Lines(Vec<usize>, UnitLocation),
}
