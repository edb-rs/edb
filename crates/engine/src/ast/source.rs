use foundry_compilers::artifacts::ast::SourceLocation;
use serde::{Deserialize, Serialize};

/// A source range is a range of characters in a source file.
///
/// `SourceRange` is semantically similar to `SourceLocation` in the `foundry-compilers` crate, but with some customized behaviors and bug fixes:
/// - The original `SourceLocation` on AST may not include the semicolon for a statement. In constract, `SourceRange` includes it. TODO: implement this
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceRange {
    /// The start index of this source range.
    pub start: usize,
    /// The length of this source range.
    pub length: usize,
    /// The index of the file of this source range.
    pub file: u32,
}

impl From<SourceRange> for SourceLocation {
    fn from(src: SourceRange) -> Self {
        SourceLocation {
            start: Some(src.start),
            length: Some(src.length),
            index: Some(src.file as usize),
        }
    }
}

impl SourceRange {
    /// The next source location (the character index in the current source file) after this source range.
    pub fn next_loc(&self) -> usize {
        self.start + self.length
    }
}
