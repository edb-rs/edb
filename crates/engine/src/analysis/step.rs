use std::{collections::BTreeMap, sync::Arc, sync::Mutex};

use derive_more::{Deref, DerefMut};
use foundry_compilers::artifacts::{
    ast::SourceLocation, BlockOrStatement, Expression, ExpressionOrVariableDeclarationStatement,
    ExpressionStatement, FunctionCall, SourceUnit, Statement,
};
use lazy_static::lazy_static;

use crate::{Visitor, Walk};

lazy_static! {
    /// The next USID to be assigned.
    pub static ref NEXT_USID: Mutex<USID> = Mutex::new(USID(0));
}

/// A Universal Step Identifier (USID) is a unique identifier for a step in contract execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct USID(u64);

impl USID {
    /// Increment the USID and return the previous value.
    pub fn inc(&mut self) -> Self {
        let v = *self;
        self.0 += 1;
        v
    }
}

impl From<USID> for u64 {
    fn from(usid: USID) -> Self {
        usid.0
    }
}

impl From<USID> for alloy_primitives::U256 {
    fn from(usid: USID) -> Self {
        Self::from(usid.0)
    }
}

/// Generate a new USID.
pub fn new_usid() -> USID {
    let mut usid = NEXT_USID.lock().unwrap();
    usid.inc()
}

/// A reference to a source step.
pub type SourceStepRef = Arc<SourceStep>;

/// The variant types for source steps.
#[derive(Debug, Clone)]
pub enum SourceStepVariant {
    /// A single statement that is executed in a single debug step.
    Statement(Statement),
    /// A consecutive list of statements that are executed in a single debug step.
    Statements(Vec<Statement>),
    /// The condition of an if statement that is executed in a single debug step.
    IfCondition(Expression),
    /// The header of a for loop that is executed in a single debug step.
    ForLoop(
        (
            Option<ExpressionOrVariableDeclarationStatement>,
            Option<Expression>,
            Option<ExpressionStatement>,
        ),
    ),
    /// The condition of a while loop that is executed in a single debug step.
    WhileLoop(Expression),
    /// The try external call
    Try(FunctionCall),
}

/// A set of source code statements or lines that are executed in a single debugger step.
///
/// A piece of code is a [SourceStep] if:
/// - new variables may be declared, or
/// - some variables may be assigned, or
/// - some side-effects may occur
///
/// We aim to partition the source code into a set of [SourceStep]s as fine-grained as
/// possible, each [SourceStep] is as small as possible.
#[derive(Debug, Clone)]
pub struct SourceStep {
    /// The unique identifier for this step.
    pub usid: USID,
    /// The variant of this step.
    pub variant: SourceStepVariant,
    /// The source location of this step.
    pub source_location: SourceLocation,
}

impl SourceStep {
    /// Creates a new SourceStep with the given variant and source location.
    pub fn new(variant: SourceStepVariant, source_location: SourceLocation) -> Self {
        Self { usid: new_usid(), variant, source_location }
    }

    /// Returns the variant name of this step.
    ///
    /// # Returns
    ///
    /// A string slice representing the variant name.
    pub fn variant_name(&self) -> &'static str {
        match &self.variant {
            SourceStepVariant::Statement(_) => "Statement",
            SourceStepVariant::Statements(_) => "Statements",
            SourceStepVariant::IfCondition(_) => "IfCondition",
            SourceStepVariant::ForLoop(_) => "ForLoop",
            SourceStepVariant::WhileLoop(_) => "WhileLoop",
            SourceStepVariant::Try(_) => "Try",
        }
    }
}

/// A collection of source steps representing the execution flow of a Solidity contract.
///
/// This struct encapsulates a sequence of debugger steps that can be executed
/// sequentially to trace through the execution of a Solidity contract.
#[derive(Debug, Default, Deref, DerefMut)]
pub struct SourceSteps {
    /// The collection of identified source steps.
    #[deref]
    #[deref_mut]
    steps: Vec<SourceStep>,
}

impl SourceSteps {
    /// Creates a new, empty SourceSteps collection.
    ///
    /// This constructor initializes an empty collection that is ready to
    /// collect steps during AST traversal.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a reference to the collected steps.
    ///
    /// This method provides controlled access to the internal steps collection
    /// while maintaining encapsulation.
    ///
    /// # Returns
    ///
    /// A reference to the vector of collected `SourceStep`s.
    pub fn steps(&self) -> &[SourceStep] {
        &self.steps
    }

    /// Consumes the SourceSteps and returns the steps as a vector.
    ///
    /// # Returns
    ///
    /// A vector of `SourceStep`s.
    pub fn into_steps(self) -> Vec<SourceStep> {
        self.steps
    }

    /// Returns the number of steps in the collection.
    ///
    /// # Returns
    ///
    /// The number of steps as a `usize`.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Returns true if the collection is empty.
    ///
    /// # Returns
    ///
    /// `true` if there are no steps, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Partitions a SourceUnit into a SourceSteps collection.
    ///
    /// This function creates a new StepPartitioner, walks through the AST using the visitor pattern,
    /// and returns the collected steps as a SourceSteps instance.
    ///
    /// # Arguments
    ///
    /// * `source_unit` - A reference to the SourceUnit to partition
    ///
    /// # Returns
    ///
    /// A `SourceSteps` instance containing all the steps found in the source unit.
    ///
    /// # Errors
    ///
    /// Returns an error if the AST walking process fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use foundry_compilers::artifacts::SourceUnit;
    /// use crate::analysis::step::SourceSteps;
    ///
    /// let source_unit: SourceUnit = /* compiled contract */;
    /// let steps = SourceSteps::partition_from(&source_unit)?;
    ///
    /// println!("Found {} debug steps", steps.len());
    /// ```
    pub fn partition_from(source_unit: &SourceUnit) -> eyre::Result<Self> {
        let mut partitioner = StepPartitioner::default();
        source_unit.walk(&mut partitioner)?;
        Ok(Self { steps: partitioner.steps })
    }

    /// Pretty prints the source steps with source location information.
    ///
    /// This method takes a source string and outputs a formatted string
    /// that shows each step with its type, source location, and the actual source code snippet.
    ///
    /// # Arguments
    ///
    /// * `sources` - A BTreeMap mapping source indices to source code strings
    ///
    /// # Returns
    ///
    /// A formatted string suitable for console output
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    /// use crate::analysis::step::SourceSteps;
    ///
    /// let steps = SourceSteps::new();
    /// let sources = BTreeMap::new();
    /// let output = steps.pretty_print(&sources);
    /// println!("{}", output);
    /// ```
    pub fn pretty_print(&self, sources: &BTreeMap<usize, String>) -> String {
        let mut output = String::new();

        if self.steps.is_empty() {
            output.push_str("No steps found.\n");
            return output;
        }

        output.push_str(&format!("Found {} step(s):\n", self.steps.len()));
        output.push_str(&"=".repeat(50));
        output.push('\n');

        for (i, step) in self.steps.iter().enumerate() {
            output.push_str(&format!("\nStep {}: ", i + 1));

            // Get the source location and extract the code snippet
            let step_type = step.variant_name();
            let location = &step.source_location;

            output.push_str(step_type);

            // Extract and display the source code snippet
            let source = sources.get(&location.index.unwrap_or(0)).expect("Source not found");
            if let (Some(start), Some(length)) = (location.start, location.length) {
                let end = start + length;
                if end <= source.len() {
                    let snippet = &source[start..end];
                    // Clean up the snippet by removing extra whitespace
                    let cleaned_snippet = snippet.trim();
                    if !cleaned_snippet.is_empty() {
                        output.push_str(&format!(
                            "\n  Location: {}:{}-{}",
                            location.index.unwrap_or(0),
                            start,
                            end
                        ));
                        output.push_str(&format!("\n  Code: \"{cleaned_snippet}\""));
                    }
                }
            }

            output.push('\n');
        }

        output.push_str(&"=".repeat(50));
        output.push('\n');

        output
    }
}

/// A partitioner that analyzes Solidity source code and breaks it down into executable steps.
///
/// The `StepPartitioner` is responsible for traversing the Abstract Syntax Tree (AST) of a Solidity
/// contract and identifying discrete execution steps. Each step represents a single debugger step
/// that can be executed atomically.
///
/// ## Purpose
///
/// The partitioner serves as a bridge between the high-level Solidity source code and the low-level
/// debugger execution model. It identifies:
/// - Variable declarations and assignments
/// - Control flow statements (if, while, for, etc.)
/// - Function calls and expressions
/// - Error handling constructs (try-catch, revert)
/// - Loop conditions and iterations
///
/// ## How it works
///
/// 1. **AST Traversal**: Uses the visitor pattern to walk through the Solidity AST
/// 2. **Step Identification**: Identifies code segments that represent single debugger steps
/// 3. **Location Tracking**: Preserves source location information for each step
/// 4. **Granular Partitioning**: Aims to create the finest possible granularity for debugging
///
/// ## Usage
///
/// ```rust
/// use foundry_compilers::artifacts::SourceUnit;
/// use crate::analysis::step::SourceSteps;
///
/// // Assuming you have a compiled SourceUnit
/// let source_unit: SourceUnit = /* ... */;
///
/// // Partition the source unit into steps
/// let steps = SourceSteps::partition_from(&source_unit)?;
///
/// // Each step can be executed individually in the debugger
/// for (i, step) in steps.steps().iter().enumerate() {
///     println!("Step {}: {:?}", i, step);
/// }
/// ```
///
/// ## Step Types
///
/// The partitioner identifies several types of steps:
/// - **Statement**: Single statements like variable declarations, assignments, etc.
/// - **Statements**: Consecutive statements executed together
/// - **IfCondition**: The condition part of an if statement
/// - **ForLoop**: The header of a for loop (initialization, condition, increment)
/// - **WhileLoop**: The condition of a while or do-while loop
/// - **Try**: External function calls in try-catch blocks
///
/// ## Example
///
/// For the following Solidity code:
/// ```solidity
/// function example() public {
///     uint256 a = 1;
///     if (a > 0) {
///         a = a + 1;
///     }
///     for (uint256 i = 0; i < 5; i++) {
///         a = a * 2;
///     }
/// }
/// ```
///
/// The partitioner would identify these steps:
/// 1. Variable declaration: `uint256 a = 1;`
/// 2. If condition: `if (a > 0)`
/// 3. Assignment: `a = a + 1;`
/// 4. For loop header: `for (uint256 i = 0; i < 5; i++)`
/// 5. Assignment: `a = a * 2;`
///
/// ## Integration with Debugger
///
/// The steps produced by this partitioner are designed to work seamlessly with the debugger:
/// - Each step corresponds to a single debugger breakpoint
/// - Source locations are preserved for accurate debugging
/// - Steps maintain the original execution order
/// - Control flow is properly represented
#[derive(Debug, Default)]
struct StepPartitioner {
    /// The collection of identified source steps.
    ///
    /// This field is private to enforce encapsulation. Use the `steps()` method
    /// to access the collected steps.
    steps: Vec<SourceStep>,
}

impl Visitor for StepPartitioner {
    fn visit_statement(&mut self, statement: &Statement) -> eyre::Result<()> {
        macro_rules! step {
            ($variant:ident, $stmt:expr, $loc:expr) => {{
                let step = SourceStep::new(SourceStepVariant::$variant($stmt.clone()), $loc);
                self.steps.push(step);
            }};
        }
        macro_rules! simple_stmt_to_step {
            ($stmt:expr) => {
                step!(Statement, statement, $stmt.src)
            };
        }
        match statement {
            Statement::Block(_) => {}
            Statement::Break(break_stmt) => simple_stmt_to_step!(break_stmt),
            Statement::Continue(continue_stmt) => simple_stmt_to_step!(continue_stmt),
            Statement::DoWhileStatement(do_while_statement) => {
                // the step is the `while(...)`
                let loc = sloc_rdiff(do_while_statement.src, do_while_statement.body.src);
                step!(WhileLoop, do_while_statement.condition.clone(), loc);
            }
            Statement::EmitStatement(emit_statement) => simple_stmt_to_step!(emit_statement),
            Statement::ExpressionStatement(expr_stmt) => simple_stmt_to_step!(expr_stmt),
            Statement::ForStatement(for_statement) => {
                // the step is the `for(...)`
                let loc = sloc_ldiff(for_statement.src, block_or_stmt_src(&for_statement.body));
                step!(
                    ForLoop,
                    (
                        for_statement.initialization_expression.clone(),
                        for_statement.condition.clone(),
                        for_statement.loop_expression.clone()
                    ),
                    loc
                );
            }
            Statement::IfStatement(if_statement) => {
                // the step is the `if(...)`
                let loc = sloc_ldiff(if_statement.src, block_or_stmt_src(&if_statement.true_body));
                step!(IfCondition, if_statement.condition.clone(), loc);
            }
            Statement::InlineAssembly(inline_assembly) => simple_stmt_to_step!(inline_assembly),
            Statement::PlaceholderStatement(placeholder_statement) => {
                simple_stmt_to_step!(placeholder_statement)
            }
            Statement::Return(return_stmt) => simple_stmt_to_step!(return_stmt),
            Statement::RevertStatement(revert_statement) => simple_stmt_to_step!(revert_statement),
            Statement::TryStatement(try_statement) => {
                // the step is the `try`
                let first_clause = &try_statement.clauses[0];
                let loc = sloc_ldiff(try_statement.src, first_clause.block.src);
                step!(Try, try_statement.external_call.clone(), loc);
            }
            Statement::UncheckedBlock(_) => { /* walk in the block */ }
            Statement::VariableDeclarationStatement(variable_declaration_statement) => {
                simple_stmt_to_step!(variable_declaration_statement)
            }
            Statement::WhileStatement(while_statement) => {
                // the step is the `while(...)`
                let loc = sloc_rdiff(while_statement.src, block_or_stmt_src(&while_statement.body));
                step!(WhileLoop, while_statement.condition.clone(), loc);
            }
        };
        Ok(())
    }
}

/// Computes the left difference of `a` and `b` (`a \ b`).
/// It takes the [SourceLocation] within `a` that is not in `b` and smaller than `b`.
fn sloc_ldiff(a: SourceLocation, b: SourceLocation) -> SourceLocation {
    assert_eq!(a.index, b.index, "The index of `a` and `b` must be the same");
    let length = b.start.zip(a.start).map(|(end, start)| end.saturating_sub(start));
    SourceLocation { start: a.start, length, index: a.index }
}

/// Computes the right difference of `a` and `b` (`a \ b`).
/// It takes the [SourceLocation] within `a` that is not in `b` and larger than `b`.
fn sloc_rdiff(a: SourceLocation, b: SourceLocation) -> SourceLocation {
    assert_eq!(a.index, b.index, "The index of `a` and `b` must be the same");
    let start = b.start.zip(b.length).map(|(start, length)| start + length);
    let length = a
        .start
        .zip(a.length)
        .map(|(start, length)| start + length)
        .zip(start)
        .map(|(end, start)| end.saturating_sub(start));
    SourceLocation { start, length, index: a.index }
}

/// Returns the source location of [Statement].
fn stmt_src(stmt: &Statement) -> SourceLocation {
    match stmt {
        Statement::Block(block) => block.src,
        Statement::ExpressionStatement(expression_statement) => expression_statement.src,
        Statement::Break(break_stmt) => break_stmt.src,
        Statement::Continue(continue_stmt) => continue_stmt.src,
        Statement::DoWhileStatement(do_while_statement) => do_while_statement.src,
        Statement::EmitStatement(emit_statement) => emit_statement.src,
        Statement::ForStatement(for_statement) => for_statement.src,
        Statement::IfStatement(if_statement) => if_statement.src,
        Statement::InlineAssembly(inline_assembly) => inline_assembly.src,
        Statement::PlaceholderStatement(placeholder_statement) => placeholder_statement.src,
        Statement::Return(return_stmt) => return_stmt.src,
        Statement::RevertStatement(revert_statement) => revert_statement.src,
        Statement::TryStatement(try_statement) => try_statement.src,
        Statement::UncheckedBlock(unchecked_block) => unchecked_block.src,
        Statement::VariableDeclarationStatement(variable_declaration_statement) => {
            variable_declaration_statement.src
        }
        Statement::WhileStatement(while_statement) => while_statement.src,
    }
}

/// Returns the source location of [BlockOrStatement].
fn block_or_stmt_src(block_or_stmt: &BlockOrStatement) -> SourceLocation {
    match block_or_stmt {
        BlockOrStatement::Block(block) => block.src,
        BlockOrStatement::Statement(statement) => stmt_src(statement),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::compile_contract_source_to_source_unit;
    use semver::Version;

    /// Utility function to count step types from a vector of SourceSteps.
    ///
    /// This function iterates through the steps and categorizes them by type,
    /// returning a HashMap with step type names as keys and counts as values.
    ///
    /// # Arguments
    ///
    /// * `steps` - A slice of SourceSteps to analyze
    ///
    /// # Returns
    ///
    /// A HashMap containing step type names and their counts
    fn count_step_types(steps: &SourceSteps) -> std::collections::HashMap<String, usize> {
        let mut step_types = std::collections::HashMap::new();

        for step in steps.iter() {
            let type_name = match &step.variant {
                SourceStepVariant::Statement(statement) => match statement {
                    Statement::VariableDeclarationStatement(_) => "VariableDeclarationStatement",
                    Statement::Break(_) => "Break",
                    Statement::Continue(_) => "Continue",
                    Statement::ExpressionStatement(_) => "ExpressionStatement",
                    Statement::EmitStatement(_) => "EmitStatement",
                    Statement::InlineAssembly(_) => "InlineAssembly",
                    Statement::Return(_) => "Return",
                    Statement::RevertStatement(_) => "RevertStatement",
                    Statement::TryStatement(_) => "Try",
                    _ => "OtherStatement",
                },
                SourceStepVariant::WhileLoop(_) => "WhileLoop",
                SourceStepVariant::ForLoop(_) => "ForLoop",
                SourceStepVariant::IfCondition(_) => "IfCondition",
                SourceStepVariant::Try(_) => "Try",
                _ => "Other",
            };

            *step_types.entry(type_name.to_string()).or_insert(0) += 1;
        }

        step_types
    }

    macro_rules! sloc {
        ($start:expr, $length:expr, $index:expr) => {
            SourceLocation { start: Some($start), length: Some($length), index: Some($index) }
        };
    }

    #[test]
    fn test_sloc_ldiff() {
        let a = sloc!(0, 10, 0);
        let b = sloc!(5, 5, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(0, 5, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(0, 0, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(10, 10, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(0, 10, 0));

        let a = sloc!(5, 5, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_ldiff(a, b);
        assert_eq!(c, sloc!(5, 0, 0));
    }

    #[test]
    fn test_sloc_rdiff() {
        let a = sloc!(0, 10, 0);
        let b = sloc!(5, 5, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(10, 0, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(10, 0, 0));

        let a = sloc!(0, 10, 0);
        let b = sloc!(0, 5, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(5, 5, 0));

        let a = sloc!(5, 5, 0);
        let b = sloc!(0, 10, 0);
        let c = sloc_rdiff(a, b);
        assert_eq!(c, sloc!(10, 0, 0));
    }

    #[test]
    fn test_partition_function_with_three_sequential_statements() {
        // Create a Solidity contract with a function containing three sequential simple statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;
        value = a + b;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the step partitioning using the new SourceSteps::partition_from method
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // Now that ExpressionStatement is also implemented, we expect 3 steps
        // (the two variable declarations: uint256 a = 1; and uint256 b = 2;)
        // plus the expression statement: value = a + b;
        assert_eq!(
            steps.len(),
            3,
            "Should have collected 3 steps from variable declarations and expression"
        );
    }

    #[test]
    fn test_step_partitioner_partition_function() {
        // Create a Solidity contract with a function containing three sequential simple statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;
        value = a + b;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the new partition_from function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // Now that ExpressionStatement is also implemented, we expect 3 steps
        // (the two variable declarations: uint256 a = 1; and uint256 b = 2;)
        // plus the expression statement: value = a + b;
        assert_eq!(
            steps.len(),
            3,
            "Should have collected 3 steps from variable declarations and expression"
        );

        // Verify that the steps are of the expected type using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &2,
            "Should have 2 variable declarations"
        );
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &1,
            "Should have 1 expression statement"
        );
    }

    #[test]
    fn test_partition_function_with_do_while_loop() {
        // Create a Solidity contract with a function containing a do-while loop
        // that has break and continue statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;

        do {
            if (a > 10) {
                break;
            }
            if (b < 5) {
                continue;
            }
            value = a + b;
            a++;
        } while (a < 20);

        value = a * b;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the partition function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // We expect the following steps:
        // 1. VariableDeclarationStatement: uint256 a = 1;
        // 2. VariableDeclarationStatement: uint256 b = 2;
        // 3. WhileLoop: the do-while condition (a < 20)
        // 4. IfCondition: if (a > 10)
        // 5. Break: break statement
        // 6. IfCondition: if (b < 5)
        // 7. Continue: continue statement
        // 8. Statement: value = a + b; (ExpressionStatement)
        // 9. Statement: a++; (ExpressionStatement)
        // 10. Statement: value = a * b; (ExpressionStatement)

        // Now that ExpressionStatement is implemented, we expect all 10 steps
        // 2 variable declarations + 1 while loop + 2 if conditions + 1 break + 1 continue + 3 expression statements = 10 steps
        assert_eq!(
            steps.len(),
            10,
            "Should have collected 10 steps from the do-while loop function"
        );

        // Verify that the steps are of the expected types using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &2,
            "Should have 2 variable declarations"
        );
        assert_eq!(step_counts.get("WhileLoop").unwrap_or(&0), &1, "Should have 1 while loop");
        assert_eq!(step_counts.get("IfCondition").unwrap_or(&0), &2, "Should have 2 if conditions");
        assert_eq!(step_counts.get("Break").unwrap_or(&0), &1, "Should have 1 break statement");
        assert_eq!(
            step_counts.get("Continue").unwrap_or(&0),
            &1,
            "Should have 1 continue statement"
        );
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &3,
            "Should have 3 expression statements"
        );
    }

    #[test]
    fn test_partition_function_with_try_statement() {
        // Create a Solidity contract with a function containing a try statement
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IExternalContract {
    function externalFunction() external returns (uint256);
}

contract TestContract {
    uint256 public value;
    IExternalContract public externalContract;

    function testFunction() public {
        uint256 a = 1;

        try externalContract.externalFunction() returns (uint256 result) {
            value = result;
        } catch {
            value = a;
        }

        value = value + 1;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the step partitioning using the partition_from function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // We expect the following steps:
        // 1. VariableDeclarationStatement: uint256 a = 1;
        // 2. Try: the try statement with external call
        // 3. Statement: value = result; (ExpressionStatement - inside try block)
        // 4. Statement: value = a; (ExpressionStatement - inside catch block)
        // 5. Statement: value = value + 1; (ExpressionStatement - after try-catch)

        assert_eq!(steps.len(), 5, "Should have collected 5 steps from try statement function");

        // Verify we have the expected step types using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &1,
            "Should have 1 variable declaration"
        );
        assert_eq!(step_counts.get("Try").unwrap_or(&0), &1, "Should have 1 try statement");
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &3,
            "Should have 3 expression statements"
        );
    }

    #[test]
    fn test_partition_function_with_for_and_emit_statements() {
        // Create a Solidity contract with a function containing a for loop and emit statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;

    event ValueUpdated(uint256 oldValue, uint256 newValue);
    event LoopIteration(uint256 index, uint256 currentValue);

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;

        emit ValueUpdated(0, a);

        for (uint256 i = 0; i < 5; i++) {
            emit LoopIteration(i, value);
            value = value + a;
        }

        emit ValueUpdated(a, value);
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the step partitioning using the partition_from function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // We expect the following steps:
        // 1. VariableDeclarationStatement: uint256 a = 1;
        // 2. VariableDeclarationStatement: uint256 b = 2;
        // 3. EmitStatement: emit ValueUpdated(0, a);
        // 4. ForLoop: the for loop header (for (uint256 i = 0; i < 5; i++))
        // 5. EmitStatement: emit LoopIteration(i, value); (inside for loop)
        // 6. ExpressionStatement: value = value + a; (inside for loop)
        // 7. EmitStatement: emit ValueUpdated(a, value);

        assert_eq!(
            steps.len(),
            7,
            "Should have collected 7 steps from for and emit statements function"
        );

        // Verify we have the expected step types using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &2,
            "Should have 2 variable declarations"
        );
        assert_eq!(
            step_counts.get("EmitStatement").unwrap_or(&0),
            &3,
            "Should have 3 emit statements"
        );
        assert_eq!(step_counts.get("ForLoop").unwrap_or(&0), &1, "Should have 1 for loop");
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &1,
            "Should have 1 expression statement"
        );
    }

    #[test]
    fn test_partition_function_with_if_return_and_revert_statements() {
        // Create a Solidity contract with a function containing if statements, return statements, and revert statements
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;
    bool public isActive;

    error InvalidInput();

    function testFunction(uint256 input) public returns (uint256) {
        uint256 a = 1;
        uint256 b = 2;

        if (input == 0) {
            revert InvalidInput();
        }

        if (input > 100) {
            return 999;
        }

        if (input < 50) {
            value = input * 2;
            return value;
        }

        value = input + a + b;
        return value;
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the step partitioning using the partition_from function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // We expect the following steps:
        // 1. VariableDeclarationStatement: uint256 a = 1;
        // 2. VariableDeclarationStatement: uint256 b = 2;
        // 3. IfCondition: if (input == 0)
        // 4. RevertStatement: revert("Input cannot be zero");
        // 5. IfCondition: if (input > 100)
        // 6. Return: return 999;
        // 7. IfCondition: if (input < 50)
        // 8. ExpressionStatement: value = input * 2;
        // 9. Return: return value;
        // 10. ExpressionStatement: value = input + a + b;
        // 11. Return: return value;

        assert_eq!(
            steps.len(),
            11,
            "Should have collected 11 steps from if, return, and revert statements function"
        );

        // Verify we have the expected step types using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &2,
            "Should have 2 variable declarations"
        );
        assert_eq!(step_counts.get("IfCondition").unwrap_or(&0), &3, "Should have 3 if conditions");
        assert_eq!(
            step_counts.get("RevertStatement").unwrap_or(&0),
            &1,
            "Should have 1 revert statement"
        );
        assert_eq!(step_counts.get("Return").unwrap_or(&0), &3, "Should have 3 return statements");
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &2,
            "Should have 2 expression statements"
        );
    }

    #[test]
    fn test_partition_function_with_while_unchecked_and_assembly() {
        // Create a Solidity contract with a function containing while loops, unchecked blocks, and inline assembly
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;
    uint256 public counter;

    function testFunction() public {
        uint256 a = 1;
        uint256 b = 2;

        // While loop with condition
        while (counter < 5) {
            value = value + a;
            counter++;
        }

        // Unchecked block for arithmetic operations
        unchecked {
            uint256 result = a + b;
            value = value + result;
        }

        // Inline assembly for low-level operations
        assembly {
            let x := 42
            sstore(0, x)
        }

        // Another while loop
        while (value > 100) {
            value = value / 2;
        }
    }
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the step partitioning using the partition_from function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // We expect the following steps:
        // 1. VariableDeclarationStatement: uint256 a = 1;
        // 2. VariableDeclarationStatement: uint256 b = 2;
        // 3. WhileLoop: while (counter < 5)
        // 4. ExpressionStatement: value = value + a; (inside while loop)
        // 5. ExpressionStatement: counter++; (inside while loop)
        // 6. VariableDeclarationStatement: uint256 result = a + b; (inside unchecked block)
        // 7. ExpressionStatement: value = value + result; (inside unchecked block)
        // 8. InlineAssembly: assembly { ... }
        // 9. WhileLoop: while (value > 100)
        // 10. ExpressionStatement: value = value / 2; (inside while loop)

        assert_eq!(
            steps.len(),
            10,
            "Should have collected 10 steps from while, unchecked, and assembly statements function"
        );

        // Verify we have the expected step types using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &3,
            "Should have 3 variable declarations"
        );
        assert_eq!(step_counts.get("WhileLoop").unwrap_or(&0), &2, "Should have 2 while loops");
        assert_eq!(
            step_counts.get("InlineAssembly").unwrap_or(&0),
            &1,
            "Should have 1 inline assembly statement"
        );
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &4,
            "Should have 4 expression statements"
        );
    }

    #[test]
    fn test_partition_function_with_multiple_statements_on_same_line() {
        // Create a Solidity contract with multiple statements on the same line
        let source = r#"
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract TestContract {
    uint256 public value;
    uint256 public counter;

    function testFunction() public returns (uint256) {
        uint256 a = 1; uint256 b = 2; uint256 c = 3;

        value = a; counter = b; value = value + c;

        if (value > 5) { value = 0; counter = 0; }

        for (uint256 i = 0; i < 3; i++) { value = value + i; counter = counter + 1; }

        while (counter < 10) { value = value * 2; counter = counter + 1; }

        emit ValueUpdated(value, counter); emit CounterIncremented(counter);

        return value;
    }

    event ValueUpdated(uint256 value, uint256 counter);
    event CounterIncremented(uint256 counter);
}
"#;

        // Compile the source code to get the AST
        let version = Version::parse("0.8.19").unwrap();
        let result = compile_contract_source_to_source_unit(version, source, false);
        assert!(result.is_ok(), "Compilation should succeed");

        let ast = result.unwrap();

        // Test the step partitioning using the partition_from function
        let steps = SourceSteps::partition_from(&ast).expect("Partitioning should succeed");

        // We expect the following steps:
        // 1. VariableDeclarationStatement: uint256 a = 1;
        // 2. VariableDeclarationStatement: uint256 b = 2;
        // 3. VariableDeclarationStatement: uint256 c = 3;
        // 4. ExpressionStatement: value = a;
        // 5. ExpressionStatement: counter = b;
        // 6. ExpressionStatement: value = value + c;
        // 7. IfCondition: if (value > 5)
        // 8. ExpressionStatement: value = 0; (inside if block)
        // 9. ExpressionStatement: counter = 0; (inside if block)
        // 10. ForLoop: for (uint256 i = 0; i < 3; i++)
        // 11. ExpressionStatement: value = value + i; (inside for loop)
        // 12. ExpressionStatement: counter = counter + 1; (inside for loop)
        // 13. WhileLoop: while (counter < 10)
        // 14. ExpressionStatement: value = value * 2; (inside while loop)
        // 15. ExpressionStatement: counter = counter + 1; (inside while loop)
        // 16. EmitStatement: emit ValueUpdated(value, counter);
        // 17. EmitStatement: emit CounterIncremented(counter);
        // 18. Return: return value;

        assert_eq!(
            steps.len(),
            18,
            "Should have collected 18 steps from function with multiple statements on same line"
        );

        // Verify we have the expected step types using the utility function
        let step_counts = count_step_types(&steps);

        assert_eq!(
            step_counts.get("VariableDeclarationStatement").unwrap_or(&0),
            &3,
            "Should have 3 variable declarations"
        );
        assert_eq!(
            step_counts.get("ExpressionStatement").unwrap_or(&0),
            &9,
            "Should have 9 expression statements"
        );
        assert_eq!(step_counts.get("IfCondition").unwrap_or(&0), &1, "Should have 1 if condition");
        assert_eq!(step_counts.get("ForLoop").unwrap_or(&0), &1, "Should have 1 for loop");
        assert_eq!(step_counts.get("WhileLoop").unwrap_or(&0), &1, "Should have 1 while loop");
        assert_eq!(
            step_counts.get("EmitStatement").unwrap_or(&0),
            &2,
            "Should have 2 emit statements"
        );
        assert_eq!(step_counts.get("Return").unwrap_or(&0), &1, "Should have 1 return statement");
    }
}
