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

use alloy_primitives::U256;

/// Format a U256 value with hex, optional decimal, and ASCII decode
pub fn format_value_with_decode(value: &U256) -> String {
    let hex_str = format!("{value:#066x}");

    // Small number decode (with consistent padding)
    let num_part = if *value < U256::from(1_000_000u64) {
        format!("({value:>7})") // Right-align in 7 chars for consistency
    } else {
        "         ".to_string() // 9 spaces to match "(1000000)" width
    };

    // ASCII decode
    let bytes = value.to_be_bytes::<32>();
    let mut decoded = String::new();

    // Only show ASCII if there are printable characters
    for byte in bytes.iter().rev() {
        // Start from least significant bytes
        if byte.is_ascii_graphic() || *byte == b' ' {
            decoded.push(*byte as char);
        } else {
            decoded.push('.');
        }
    }

    format!("{} {} {}", hex_str, num_part, decoded.chars().rev().collect::<String>())
}

/// Format a byte sequence as hex, optional decimal (if length <= 32), and ASCII decode
/// For longer byte sequences, we break into multiple lines (up to 32 bytes per line)
pub fn format_bytes_with_decode(bytes: &[u8]) -> String {
    let mut lines = Vec::new();
    for chunk in bytes.chunks(32) {
        let hex_str = chunk.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join("");
        let hex_str = format!("0x{hex_str:64}");

        let mut num = U256::ZERO;
        for &b in chunk {
            num = (num << 8) | U256::from(b);
        }

        // Small number decode (with consistent padding)
        let num_part = if num < U256::from(1_000_000u64) {
            format!("({num:>7})") // Right-align in 7 chars for consistency
        } else {
            "         ".to_string() // 9 spaces to match "(1000000)" width
        };

        let ascii_part = chunk
            .iter()
            .map(|b| if b.is_ascii_graphic() { *b as char } else { '.' })
            .collect::<String>();
        lines.push(format!("{hex_str} {num_part} {ascii_part}"));
    }
    lines.join("\n")
}
