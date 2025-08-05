//! Source code instrumentation
//!
//! This module handles instrumenting Solidity source code with precompile calls
//! to enable step-by-step debugging.

use alloy_primitives::Address;
use eyre::Result;
use std::collections::HashMap;

/// The precompile address used for instrumentation
pub const INSTRUMENTATION_PRECOMPILE: Address =
    Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02, 0x33, 0x33]);

/// Instrument source code files with debugging precompile calls
pub fn instrument_sources(sources: &HashMap<Address, String>) -> Result<HashMap<Address, String>> {
    let mut instrumented = HashMap::new();

    for (address, source) in sources {
        tracing::debug!("Instrumenting source for contract: {:?}", address);
        let instrumented_source = instrument_source(source)?;
        instrumented.insert(*address, instrumented_source);
    }

    Ok(instrumented)
}

/// Instrument a single source file
fn instrument_source(source: &str) -> Result<String> {
    // Parse the source code to find function boundaries
    let mut lines: Vec<String> = source.lines().map(String::from).collect();
    let mut modified = false;

    // Simple instrumentation: inject at the start of each function
    // This is a simplified version - a real implementation would use
    // a proper Solidity parser
    for i in 0..lines.len() {
        let line = &lines[i];
        let trimmed = line.trim();

        // Detect function definitions
        if is_function_start(trimmed) {
            // Find the opening brace
            if let Some(brace_idx) = find_function_body_start(&lines, i) {
                // Insert instrumentation after the opening brace
                let indent = get_indent(&lines[brace_idx]);
                let instrumentation = format!("{}    // EDB instrumentation", indent);
                let call = format!(
                    "{}    assembly {{ let success := call(gas(), {}, 0, 0, 0, 0, 0) }}",
                    indent,
                    format_address(&INSTRUMENTATION_PRECOMPILE)
                );

                lines.insert(brace_idx + 1, instrumentation);
                lines.insert(brace_idx + 2, call);
                modified = true;
            }
        }
    }

    if modified {
        tracing::info!("Instrumented source code with debugging calls");
    } else {
        tracing::warn!("No functions found to instrument");
    }

    Ok(lines.join("\n"))
}

/// Check if a line starts a function definition
fn is_function_start(line: &str) -> bool {
    // Simple heuristic - look for "function" keyword
    // A real implementation would use a proper parser
    line.starts_with("function ") || (line.contains("function ") && !line.contains("//"))
}

/// Find the line index where the function body starts (after the opening brace)
fn find_function_body_start(lines: &[String], start_idx: usize) -> Option<usize> {
    let mut brace_count = 0;

    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    if brace_count == 0 {
                        return Some(i);
                    }
                    brace_count += 1;
                }
                '}' => {
                    brace_count -= 1;
                }
                _ => {}
            }
        }
    }

    None
}

/// Get the indentation of a line
fn get_indent(line: &str) -> &str {
    let trimmed = line.trim_start();
    &line[..line.len() - trimmed.len()]
}

/// Format an address for Solidity
fn format_address(addr: &Address) -> String {
    format!("0x{}", hex::encode(addr.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_instrumentation() {
        let source = r#"
contract Test {
    function foo() public {
        uint x = 1;
    }
    
    function bar(uint a) external returns (uint) {
        return a * 2;
    }
}
"#;

        let result = instrument_source(source).unwrap();
        assert!(result.contains("assembly"));
        assert!(result.contains(&format_address(&INSTRUMENTATION_PRECOMPILE)));
    }
}
