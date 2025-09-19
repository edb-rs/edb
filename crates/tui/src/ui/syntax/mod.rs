// EDB - Ethereum Debugger
// Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Syntax highlighting module
//!
//! This module provides syntax highlighting for different programming languages
//! and assembly formats used in the debugger.

use ratatui::style::Style;

use crate::ColorScheme;

pub mod opcodes;
pub mod solidity;

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
    _Default, // Default text
}

/// Syntax token with position and type
#[derive(Debug, Clone)]
pub struct SyntaxToken {
    pub start: usize,
    pub end: usize,
    pub token_type: TokenType,
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

    /// Convert TokenStyle to ratatui Style using theme colors
    pub fn get_token_style(&self, token_type: TokenType, color_scheme: &ColorScheme) -> Style {
        let color = match token_type {
            TokenType::Keyword => color_scheme.syntax_keyword_color,
            TokenType::Type => color_scheme.syntax_type_color,
            TokenType::String => color_scheme.syntax_string_color,
            TokenType::Number => color_scheme.syntax_number_color,
            TokenType::Comment => color_scheme.syntax_comment_color,
            TokenType::Identifier => color_scheme.syntax_identifier_color,
            TokenType::Operator => color_scheme.syntax_operator_color,
            TokenType::Punctuation => color_scheme.syntax_punctuation_color,
            TokenType::Address => color_scheme.syntax_address_color,
            TokenType::Pragma => color_scheme.syntax_pragma_color,
            TokenType::Opcode
            | TokenType::OpcodeNumber
            | TokenType::OpcodeAddress
            | TokenType::OpcodeData => color_scheme.syntax_opcode_color,
            TokenType::_Default => color_scheme.comment_color,
        };
        Style::default().fg(color)
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
