use serde::{Deserialize, Serialize};

use super::{SourceRange, Stmt};

/// An AST block is the code wrapped by `{` and `}`, including the brackets, without leading and tailing spaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blk {
    pub src: SourceRange,
    pub body: Vec<Stmt>,
}

impl Blk {
    /// Returns the source location (char index) of the first statement in the block.
    pub fn first_stmt_loc(&self) -> usize {
        // We make sure that the first character of a block is `{`, so we can simply return the next location.
        self.src.start + 1
    }

    /// Returns the next source location (char index) of the last statement in the block.
    pub fn last_stmt_next_loc(&self) -> usize {
        if let Some(last_stmt) = self.body.last() {
            last_stmt.src().next_loc()
        } else {
            // If the block has no statements, the next statement location is the end of the block '}'.
            self.src.next_loc() - 1
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlkOrStmt {
    Blk(Box<Blk>),
    Stmt(Box<Stmt>),
}

impl BlkOrStmt {
    pub fn is_block(&self) -> bool {
        matches!(self, BlkOrStmt::Blk(_))
    }

    pub fn is_statement(&self) -> bool {
        matches!(self, BlkOrStmt::Stmt(_))
    }

    pub fn src(&self) -> SourceRange {
        match self {
            BlkOrStmt::Blk(blk) => blk.src,
            BlkOrStmt::Stmt(stmt) => stmt.src(),
        }
    }

    /// Returns the source location (char index) of the first statement in the block.
    pub fn first_stmt_loc(&self) -> usize {
        match self {
            BlkOrStmt::Blk(blk) => blk.first_stmt_loc(),
            BlkOrStmt::Stmt(stmt) => stmt.src().start,
        }
    }

    /// Returns the next source location (char index) of the last statement in the block.
    pub fn last_stmt_next_loc(&self) -> usize {
        match self {
            BlkOrStmt::Blk(blk) => blk.last_stmt_next_loc(),
            BlkOrStmt::Stmt(stmt) => stmt.src().next_loc(),
        }
    }
}
