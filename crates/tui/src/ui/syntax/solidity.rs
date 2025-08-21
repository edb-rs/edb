//! Solidity syntax highlighting
//!
//! This module provides comprehensive syntax highlighting for Solidity smart contracts
//! with semantic token recognition and theme-aware styling.

use super::{SyntaxToken, TokenType};
use regex::Regex;
use std::sync::OnceLock;

/// Solidity syntax highlighter
#[derive(Debug)]
pub struct SolidityHighlighter {
    patterns: &'static SolidityPatterns,
}

/// Solidity syntax patterns
struct SolidityPatterns {
    keywords: Regex,
    types: Regex,
    strings: Regex,
    numbers: Regex,
    comments: Regex,
    addresses: Regex,
    pragma: Regex,
    operators: Regex,
    punctuation: Regex,
    identifiers: Regex,
}

impl std::fmt::Debug for SolidityPatterns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolidityPatterns")
            .field("keywords", &"<regex>")
            .field("types", &"<regex>")
            .field("strings", &"<regex>")
            .field("numbers", &"<regex>")
            .field("comments", &"<regex>")
            .field("addresses", &"<regex>")
            .field("pragma", &"<regex>")
            .field("operators", &"<regex>")
            .field("punctuation", &"<regex>")
            .field("identifiers", &"<regex>")
            .finish()
    }
}

impl SolidityHighlighter {
    /// Create a new Solidity syntax highlighter
    pub fn new() -> Self {
        Self { patterns: get_solidity_patterns() }
    }

    /// Tokenize Solidity source code
    pub fn tokenize(&self, line: &str) -> Vec<SyntaxToken> {
        let mut tokens = Vec::new();

        // Process in order of precedence to avoid conflicts
        // Higher precedence patterns are processed first
        let patterns = [
            (&self.patterns.comments, TokenType::Comment), // Comments first (highest precedence)
            (&self.patterns.strings, TokenType::String),   // Strings second
            (&self.patterns.pragma, TokenType::Pragma),    // Pragma statements
            (&self.patterns.addresses, TokenType::Address), // Ethereum addresses
            (&self.patterns.numbers, TokenType::Number),   // Numbers
            (&self.patterns.keywords, TokenType::Keyword), // Keywords
            (&self.patterns.types, TokenType::Type),       // Type names
            (&self.patterns.operators, TokenType::Operator), // Operators
            (&self.patterns.punctuation, TokenType::Punctuation), // Punctuation
        ];

        let mut covered_ranges = Vec::new();

        // Apply patterns in priority order
        for (pattern, token_type) in patterns {
            for mat in pattern.find_iter(line) {
                let start = mat.start();
                let end = mat.end();

                // Check if this range overlaps with any existing range
                let overlaps = covered_ranges.iter().any(|(s, e)| start < *e && end > *s);

                if !overlaps {
                    tokens.push(SyntaxToken { start, end, token_type });
                    covered_ranges.push((start, end));
                }
            }
        }

        // Add identifiers for remaining uncovered word characters
        for mat in self.patterns.identifiers.find_iter(line) {
            let start = mat.start();
            let end = mat.end();

            // Check if this range is covered by existing tokens
            let covered = covered_ranges.iter().any(|(s, e)| start >= *s && end <= *e);

            if !covered {
                tokens.push(SyntaxToken { start, end, token_type: TokenType::Identifier });
            }
        }

        // Sort tokens by position
        tokens.sort_by_key(|t| t.start);
        tokens
    }
}

/// Get Solidity syntax patterns (cached)
fn get_solidity_patterns() -> &'static SolidityPatterns {
    static PATTERNS: OnceLock<SolidityPatterns> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        SolidityPatterns {
            // Solidity keywords - comprehensive list
            keywords: Regex::new(r"\b(contract|function|modifier|constructor|receive|fallback|if|else|for|while|do|return|break|continue|throw|try|catch|require|assert|revert|emit|new|delete|public|private|internal|external|pure|view|payable|nonpayable|override|virtual|abstract|interface|library|struct|enum|mapping|event|error|using|is|import|from|as|pragma|solidity|experimental|constant|immutable|storage|memory|calldata|stack|logs|indexed|anonymous|assembly|let|switch|case|default|leave|function)\b").unwrap(),

            // Solidity types - all built-in types
            types: Regex::new(r"\b(uint8|uint16|uint24|uint32|uint40|uint48|uint56|uint64|uint72|uint80|uint88|uint96|uint104|uint112|uint120|uint128|uint136|uint144|uint152|uint160|uint168|uint176|uint184|uint192|uint200|uint208|uint216|uint224|uint232|uint240|uint248|uint256|uint|int8|int16|int24|int32|int40|int48|int56|int64|int72|int80|int88|int96|int104|int112|int120|int128|int136|int144|int152|int160|int168|int176|int184|int192|int200|int208|int216|int224|int232|int240|int248|int256|int|address|bool|byte|bytes1|bytes2|bytes3|bytes4|bytes5|bytes6|bytes7|bytes8|bytes9|bytes10|bytes11|bytes12|bytes13|bytes14|bytes15|bytes16|bytes17|bytes18|bytes19|bytes20|bytes21|bytes22|bytes23|bytes24|bytes25|bytes26|bytes27|bytes28|bytes29|bytes30|bytes31|bytes32|bytes|string|fixed|ufixed)\b").unwrap(),

            // String literals - both double and single quotes with escape sequences
            strings: Regex::new(r#""([^"\\]|\\.)*"|'([^'\\]|\\.)*'"#).unwrap(),

            // Numbers - hex, decimal, scientific notation, with optional units
            numbers: Regex::new(r"\b(0x[0-9a-fA-F]+(_[0-9a-fA-F]+)*|\d+(_\d+)*\.?\d*(_\d+)*([eE][+-]?\d+(_\d+)*)?)\s*(wei|gwei|ether|seconds|minutes|hours|days|weeks|years)?\b").unwrap(),

            // Comments - single line and multi-line
            comments: Regex::new(r"//.*$|/\*[\s\S]*?\*/").unwrap(),

            // Ethereum addresses - exactly 40 hex characters
            addresses: Regex::new(r"\b0x[0-9a-fA-F]{40}\b").unwrap(),

            // Pragma statements
            pragma: Regex::new(r"\bpragma\s+\w+\s+[^;]+;").unwrap(),

            // Operators - comprehensive list including compound assignments
            operators: Regex::new(r"(\+\+|--|<=|>=|==|!=|&&|\|\||=>|->|\+=|-=|\*=|/=|%=|\|=|&=|\^=|<<=|>>=|<<|>>|[+\-*/%&|^~<>=!])").unwrap(),

            // Punctuation - brackets, braces, semicolons, etc.
            punctuation: Regex::new(r"[{}()\[\];,.]").unwrap(),

            // Identifiers - valid Solidity identifiers (not matched by other patterns)
            identifiers: Regex::new(r"\b[a-zA-Z_$][a-zA-Z0-9_$]*\b").unwrap(),
        }
    })
}

impl Default for SolidityHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solidity_keywords() {
        let highlighter = SolidityHighlighter::new();
        let tokens = highlighter.tokenize("contract SimpleToken {");

        assert!(!tokens.is_empty());
        let keyword_token = tokens.iter().find(|t| t.token_type == TokenType::Keyword);
        assert!(keyword_token.is_some());
        assert_eq!(keyword_token.unwrap().start, 0);
        assert_eq!(keyword_token.unwrap().end, 8); // "contract".len()
    }

    #[test]
    fn test_solidity_types() {
        let highlighter = SolidityHighlighter::new();
        let tokens = highlighter.tokenize("uint256 public balance;");

        let type_token = tokens.iter().find(|t| t.token_type == TokenType::Type);
        assert!(type_token.is_some());
    }

    #[test]
    fn test_solidity_strings() {
        let highlighter = SolidityHighlighter::new();
        let tokens = highlighter.tokenize(r#"string memory name = "Hello World";"#);

        let string_token = tokens.iter().find(|t| t.token_type == TokenType::String);
        assert!(string_token.is_some());
    }

    #[test]
    fn test_solidity_addresses() {
        let highlighter = SolidityHighlighter::new();
        let tokens =
            highlighter.tokenize("address owner = 0x742d35Cc6634C0532925a3b8D73eb14b9d7D6c5A;");

        let address_token = tokens.iter().find(|t| t.token_type == TokenType::Address);
        assert!(address_token.is_some());
    }

    #[test]
    fn test_solidity_comments() {
        let highlighter = SolidityHighlighter::new();

        // Single line comment
        let tokens = highlighter.tokenize("// This is a comment");
        let comment_token = tokens.iter().find(|t| t.token_type == TokenType::Comment);
        assert!(comment_token.is_some());

        // Multi-line comment
        let tokens = highlighter.tokenize("/* This is a multi-line comment */");
        let comment_token = tokens.iter().find(|t| t.token_type == TokenType::Comment);
        assert!(comment_token.is_some());
    }

    #[test]
    fn test_complex_solidity_line() {
        let highlighter = SolidityHighlighter::new();
        let tokens = highlighter
            .tokenize("function transfer(address to, uint256 amount) public returns (bool) {");

        // Should have multiple token types
        let has_keyword = tokens.iter().any(|t| t.token_type == TokenType::Keyword);
        let has_type = tokens.iter().any(|t| t.token_type == TokenType::Type);
        let has_identifier = tokens.iter().any(|t| t.token_type == TokenType::Identifier);
        let has_punctuation = tokens.iter().any(|t| t.token_type == TokenType::Punctuation);

        assert!(has_keyword);
        assert!(has_type);
        assert!(has_identifier);
        assert!(has_punctuation);
    }
}
