//! This module abstract the AST nodes of `foundry_compilers` to keep only the fields that we care about in EDB.

mod blk;
pub use blk::*;

mod func;
pub use func::*;

mod source;
pub use source::*;

mod stmt;
pub use stmt::*;
