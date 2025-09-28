use itertools::Either;
use serde::Deserialize;
use serde::Serialize;

use crate::BlkOrStmt;

use super::Blk;
use super::SourceRange;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    If(Box<IfStmt>),
    Loop(Box<LoopStmt>),
    Try(Box<TryStmt>),
    Jump(JumpStmt),
    Emit(EmitStmt),
    Declaration(VariableDeclarationStmt),
    Expression(ExpressionStmt),
    InlineAssembly(InlineAssembly),
}

impl Stmt {
    pub fn src(&self) -> SourceRange {
        match self {
            Stmt::If(if_stmt) => if_stmt.src,
            Stmt::Loop(loop_stmt) => loop_stmt.src,
            Stmt::Try(try_stmt) => try_stmt.src,
            Stmt::Jump(jump_stmt) => jump_stmt.src,
            Stmt::Emit(emit_stmt) => emit_stmt.src,
            Stmt::Declaration(declaration_stmt) => declaration_stmt.src,
            Stmt::Expression(expression_stmt) => expression_stmt.src,
            Stmt::InlineAssembly(inline_assembly) => inline_assembly.src,
        }
    }
}

/// Abstracted if statement AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStmt {
    pub src: SourceRange,
    pub true_branch: BlkOrStmt,
    pub false_branch: Option<BlkOrStmt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopStmt {
    pub src: SourceRange,
    pub body: BlkOrStmt,
    pub is_do_while: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TryStmt {
    pub src: SourceRange,
    pub clauses: Vec<Blk>,
}

/// Jump-like statement, such as `return`, `break`, `continue`, `revert`, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpStmt {
    pub src: SourceRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclarationStmt {
    pub src: SourceRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionStmt {
    pub src: SourceRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmitStmt {
    pub src: SourceRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineAssembly {
    pub src: SourceRange,
}
