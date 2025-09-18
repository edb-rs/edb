use alloy_primitives::U256;

/// Format a U256 value with hex, optional decimal, and ASCII decode
pub fn format_value_with_decode(value: &U256) -> String {
    let hex_str = format!("{:#066x}", value);

    // Small number decode (with consistent padding)
    let num_part = if *value < U256::from(1_000_000u64) {
        format!("({:>7})", value) // Right-align in 7 chars for consistency
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
        let hex_str = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("");
        let hex_str = format!("0x{:64}", hex_str);

        let mut num = U256::ZERO;
        for &b in chunk {
            num = (num << 8) | U256::from(b);
        }

        // Small number decode (with consistent padding)
        let num_part = if num < U256::from(1_000_000u64) {
            format!("({:>7})", num) // Right-align in 7 chars for consistency
        } else {
            "         ".to_string() // 9 spaces to match "(1000000)" width
        };

        let ascii_part = chunk
            .iter()
            .map(|b| if b.is_ascii_graphic() { *b as char } else { '.' })
            .collect::<String>();
        lines.push(format!("{} {} {}", hex_str, num_part, ascii_part));
    }
    lines.join("\n")
}
