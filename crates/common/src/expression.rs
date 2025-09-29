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

/// Normalize an expression by replacing any contiguous whitespace with a single space
pub fn normalize_expression(expr: &str) -> String {
    expr.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_expression_single_space() {
        assert_eq!(normalize_expression("a b c"), "a b c");
    }

    #[test]
    fn test_normalize_expression_multiple_spaces() {
        assert_eq!(normalize_expression("a  b    c"), "a b c");
    }

    #[test]
    fn test_normalize_expression_tabs() {
        assert_eq!(normalize_expression("a\tb\t\tc"), "a b c");
    }

    #[test]
    fn test_normalize_expression_newlines() {
        assert_eq!(normalize_expression("a\nb\n\nc"), "a b c");
    }

    #[test]
    fn test_normalize_expression_mixed_whitespace() {
        assert_eq!(normalize_expression("a  \t\n  b \r\n c"), "a b c");
    }

    #[test]
    fn test_normalize_expression_leading_trailing_whitespace() {
        assert_eq!(normalize_expression("  a b c  "), "a b c");
        assert_eq!(normalize_expression("\t\na b c\n\t"), "a b c");
    }

    #[test]
    fn test_normalize_expression_empty_string() {
        assert_eq!(normalize_expression(""), "");
    }

    #[test]
    fn test_normalize_expression_only_whitespace() {
        assert_eq!(normalize_expression("   "), "");
        assert_eq!(normalize_expression("\t\n\r"), "");
    }

    #[test]
    fn test_normalize_expression_single_word() {
        assert_eq!(normalize_expression("word"), "word");
        assert_eq!(normalize_expression("  word  "), "word");
    }

    #[test]
    fn test_normalize_expression_complex() {
        assert_eq!(
            normalize_expression("function  \n\t  test(uint256   a,\n    address b)"),
            "function test(uint256 a, address b)"
        );
    }

    #[test]
    fn test_normalize_expression_preserves_single_spaces() {
        assert_eq!(
            normalize_expression("already normalized expression"),
            "already normalized expression"
        );
    }

    #[test]
    fn test_normalize_expression_unicode_whitespace() {
        // Tests with non-ASCII whitespace characters
        assert_eq!(normalize_expression("a\u{00A0}b"), "a b"); // non-breaking space
        assert_eq!(normalize_expression("a\u{2003}b"), "a b"); // em space
    }
}
