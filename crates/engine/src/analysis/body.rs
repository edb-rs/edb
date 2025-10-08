use foundry_compilers::artifacts::{BlockOrStatement, Statement};
use serde::{Deserialize, Serialize};

use crate::analysis::{Analyzer, SourceRange};

/// A body containing a single statement without a wrapping block.
/// This struct is meant to capture loop or if statements with single statement as their bodies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementBody {
    /// The source range of the body.
    pub range: SourceRange,
}

impl StatementBody {
    /// Creates a new statement body.
    pub fn new(range: SourceRange) -> Self {
        Self { range }
    }
}

/// Creates a source range for a [`BlockOrStatement`]. This function will regulate the range by including the semicolon of statements.
pub(super) fn block_or_statement_rage(
    source: &str,
    block_or_statement: &BlockOrStatement,
) -> SourceRange {
    match block_or_statement {
        BlockOrStatement::Block(block) => block.src.into(),
        BlockOrStatement::Statement(statement) => statement_range(source, statement),
    }
}

/// Creates a source range for a [`Statement`]. This function will regulate the range by including the semicolon of statements.
pub(super) fn statement_range(source: &str, stmt: &Statement) -> SourceRange {
    macro_rules! expand_to_semicolon {
        ($stmt:expr) => {{
            let range: SourceRange = $stmt.src.into();
            let range = range.expand_to_next_semicolon(source);
            range
        }};
    }
    match stmt {
        Statement::Block(block) => block.src.into(),
        Statement::Break(stmt) => expand_to_semicolon!(stmt),
        Statement::Continue(stmt) => expand_to_semicolon!(stmt),
        Statement::DoWhileStatement(stmt) => stmt.src.into(),
        Statement::EmitStatement(stmt) => expand_to_semicolon!(stmt),
        Statement::ExpressionStatement(stmt) => expand_to_semicolon!(stmt),
        Statement::ForStatement(stmt) => {
            let range: SourceRange = stmt.src.into();
            let body_range = block_or_statement_rage(source, &stmt.body);
            range.merge(body_range)
        }
        Statement::IfStatement(stmt) => {
            let range: SourceRange = stmt.src.into();
            if let Some(false_body) = &stmt.false_body {
                let body_range = block_or_statement_rage(source, false_body);
                range.merge(body_range)
            } else {
                let body_range = block_or_statement_rage(source, &stmt.true_body);
                range.merge(body_range)
            }
        }
        Statement::InlineAssembly(stmt) => stmt.src.into(),
        Statement::PlaceholderStatement(stmt) => stmt.src.into(),
        Statement::Return(stmt) => expand_to_semicolon!(stmt),
        Statement::RevertStatement(stmt) => expand_to_semicolon!(stmt),
        Statement::TryStatement(stmt) => stmt.src.into(),
        Statement::UncheckedBlock(stmt) => stmt.src.into(),
        Statement::VariableDeclarationStatement(stmt) => expand_to_semicolon!(stmt),
        Statement::WhileStatement(stmt) => {
            let range: SourceRange = stmt.src.into();
            let body_range = block_or_statement_rage(source, &stmt.body);
            range.merge(body_range)
        }
    }
}

impl Analyzer {
    /// Collect single statement bodies in [`BlockOrStatement`]. If it is a block, it will be skipped.
    pub(super) fn collect_statement_bodies(&mut self, body: &BlockOrStatement) {
        let range = match body {
            BlockOrStatement::Statement(statement) => match statement {
                Statement::Block(_) | Statement::UncheckedBlock(_) => return,
                _ => statement_range(&self.source, statement),
            },
            BlockOrStatement::Block(_) => return,
        };
        self.statement_bodies.push(StatementBody::new(range));
    }
}
