//! Syntax highlighting module
//!
//! This module provides syntax highlighting for different programming languages
//! and assembly formats used in the debugger.

pub mod opcodes;
pub mod solidity;

use ratatui::style::Style;

/// Syntax highlighting types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxType {
    /// Solidity source code
    Solidity,
    /// Ethereum opcodes
    Opcodes,
}

/// Token types for syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    // Solidity tokens
    Keyword,     // contract, function, if, else, etc.
    Type,        // uint256, address, bool, etc.
    String,      // String literals
    Number,      // Numeric literals
    Comment,     // Comments
    Identifier,  // Variable names, function names
    Operator,    // =, +, -, *, etc.
    Punctuation, // (, ), {, }, ;, etc.
    Address,     // 0x... addresses
    Pragma,      // pragma statements

    // Opcode tokens
    Opcode,        // PUSH, POP, ADD, etc.
    OpcodeNumber,  // Numbers in opcodes
    OpcodeAddress, // Memory/stack addresses
    OpcodeData,    // Hex data

    // Common
    Default, // Default text
}

/// Syntax token with position and type
#[derive(Debug, Clone)]
pub struct SyntaxToken {
    pub start: usize,
    pub end: usize,
    pub token_type: TokenType,
}

/// Token style categories (will be themed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenStyle {
    Keyword,
    Type,
    String,
    Number,
    Comment,
    Identifier,
    Operator,
    Punctuation,
    Address,
    Pragma,
    Opcode,
    OpcodeNumber,
    OpcodeAddress,
    OpcodeData,
    Default,
}

/// Main syntax highlighter that delegates to specific language highlighters
#[derive(Debug)]
pub struct SyntaxHighlighter {
    solidity_highlighter: solidity::SolidityHighlighter,
    opcode_highlighter: opcodes::OpcodeHighlighter,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter
    pub fn new() -> Self {
        Self {
            solidity_highlighter: solidity::SolidityHighlighter::new(),
            opcode_highlighter: opcodes::OpcodeHighlighter::new(),
        }
    }

    /// Tokenize a line of code
    pub fn tokenize(&self, line: &str, syntax_type: SyntaxType) -> Vec<SyntaxToken> {
        match syntax_type {
            SyntaxType::Solidity => self.solidity_highlighter.tokenize(line),
            SyntaxType::Opcodes => self.opcode_highlighter.tokenize(line),
        }
    }

    /// Get style for a token type (theme will be applied later)
    pub fn get_token_style(&self, token_type: TokenType) -> TokenStyle {
        match token_type {
            TokenType::Keyword => TokenStyle::Keyword,
            TokenType::Type => TokenStyle::Type,
            TokenType::String => TokenStyle::String,
            TokenType::Number => TokenStyle::Number,
            TokenType::Comment => TokenStyle::Comment,
            TokenType::Identifier => TokenStyle::Identifier,
            TokenType::Operator => TokenStyle::Operator,
            TokenType::Punctuation => TokenStyle::Punctuation,
            TokenType::Address => TokenStyle::Address,
            TokenType::Pragma => TokenStyle::Pragma,
            TokenType::Opcode => TokenStyle::Opcode,
            TokenType::OpcodeNumber => TokenStyle::OpcodeNumber,
            TokenType::OpcodeAddress => TokenStyle::OpcodeAddress,
            TokenType::OpcodeData => TokenStyle::OpcodeData,
            TokenType::Default => TokenStyle::Default,
        }
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
