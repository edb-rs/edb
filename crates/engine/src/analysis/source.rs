/// A source range is a range of characters in a source file.
///
/// `SourceRange` is semantically equivalent to `SourceLocation` in the `foundry-compilers` crate, but with some customized behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceRange {
    /// The start index of this source range.
    pub start: usize,
    /// The length of this source range.
    pub length: usize,
    /// The index of the file of this source range.
    pub file: u32,
}
