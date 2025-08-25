use alloy_dyn_abi::DynSolValue;
use alloy_primitives::hex;

/// Trait for formatting Solidity values into human-readable strings.
pub trait SolValueFormatter {
    /// Formats a Solidity value into a human-readable string.
    ///
    /// # Arguments
    ///
    /// * `with_ty` - If true, includes type information in the output (e.g., "uint256(123)")
    ///
    /// # Returns
    ///
    /// A formatted string representation of the value.
    fn format_value(&self, with_ty: bool) -> String;

    /// Returns the Solidity type of this value as a string.
    ///
    /// # Returns
    ///
    /// The Solidity type (e.g., "uint256", "address", "bytes32[]")
    fn format_type(&self) -> String;
}

impl SolValueFormatter for DynSolValue {
    fn format_value(&self, with_ty: bool) -> String {
        let value_str = match self {
            DynSolValue::Bool(b) => b.to_string(),

            DynSolValue::Int(n, bits) => {
                if with_ty {
                    format!("int{}({})", bits, n)
                } else {
                    n.to_string()
                }
            }

            DynSolValue::Uint(n, bits) => {
                if with_ty {
                    format!("uint{}({})", bits, n)
                } else {
                    n.to_string()
                }
            }

            DynSolValue::Address(addr) => {
                format!("0x{:040x}", addr)
            }

            DynSolValue::Function(func) => {
                format!("0x{}", hex::encode(func.as_slice()))
            }

            DynSolValue::FixedBytes(bytes, size) => {
                if with_ty {
                    format!("bytes{}(0x{})", size, hex::encode(bytes))
                } else {
                    format!("0x{}", hex::encode(bytes))
                }
            }

            DynSolValue::Bytes(bytes) => {
                if bytes.len() <= 32 {
                    format!("0x{}", hex::encode(bytes))
                } else {
                    format!("0x{}...[{} bytes]", hex::encode(&bytes[..16]), bytes.len())
                }
            }

            DynSolValue::String(s) => {
                if s.len() <= 64 {
                    format!("\"{}\"", s.replace('\"', "\\\""))
                } else {
                    format!("\"{}...\"[{} chars]", &s[..32].replace('\"', "\\\""), s.len())
                }
            }

            DynSolValue::Array(arr) => format_array(arr, with_ty, false),

            DynSolValue::FixedArray(arr) => format_array(arr, with_ty, true),

            DynSolValue::Tuple(tuple) => format_tuple(tuple, with_ty),

            DynSolValue::CustomStruct { name, prop_names, tuple } => {
                if prop_names.is_empty() {
                    format!("{}{}", name, format_tuple(tuple, with_ty))
                } else {
                    let fields: Vec<String> = tuple
                        .iter()
                        .zip(prop_names.iter())
                        .map(|(value, name)| format!("{}: {}", name, value.format_value(with_ty)))
                        .collect();

                    if with_ty {
                        format!("{}{{ {} }}", name, fields.join(", "))
                    } else {
                        format!("{{ {} }}", fields.join(", "))
                    }
                }
            }
        };

        value_str
    }

    fn format_type(&self) -> String {
        match self {
            DynSolValue::Bool(_) => "bool".to_string(),
            DynSolValue::Int(_, bits) => format!("int{}", bits),
            DynSolValue::Uint(_, bits) => format!("uint{}", bits),
            DynSolValue::Address(_) => "address".to_string(),
            DynSolValue::Function(_) => "function".to_string(),
            DynSolValue::FixedBytes(_, size) => format!("bytes{}", size),
            DynSolValue::Bytes(_) => "bytes".to_string(),
            DynSolValue::String(_) => "string".to_string(),
            DynSolValue::Array(arr) => {
                if let Some(first) = arr.first() {
                    format!("{}[]", first.format_type())
                } else {
                    "unknown[]".to_string()
                }
            }
            DynSolValue::FixedArray(arr) => {
                if let Some(first) = arr.first() {
                    format!("{}[{}]", first.format_type(), arr.len())
                } else {
                    format!("unknown[{}]", arr.len())
                }
            }
            DynSolValue::Tuple(tuple) => {
                let types: Vec<String> = tuple.iter().map(|v| v.format_type()).collect();
                format!("({})", types.join(","))
            }
            DynSolValue::CustomStruct { name, .. } => name.clone(),
        }
    }
}

fn format_array(arr: &[DynSolValue], with_ty: bool, is_fixed: bool) -> String {
    const MAX_DISPLAY_ITEMS: usize = 5;

    if arr.is_empty() {
        return "[]".to_string();
    }

    if arr.len() <= MAX_DISPLAY_ITEMS {
        let items: Vec<String> = arr.iter().map(|v| v.format_value(with_ty)).collect();
        format!("[{}]", items.join(", "))
    } else {
        let first_items: Vec<String> =
            arr.iter().take(3).map(|v| v.format_value(with_ty)).collect();

        let suffix = if is_fixed {
            format!(", ...[{} total]", arr.len())
        } else {
            format!(", ...[{} items]", arr.len())
        };

        format!("[{}{}]", first_items.join(", "), suffix)
    }
}

fn format_tuple(tuple: &[DynSolValue], with_ty: bool) -> String {
    if tuple.is_empty() {
        return "()".to_string();
    }

    if tuple.len() == 1 {
        return format!("({})", tuple[0].format_value(with_ty));
    }

    const MAX_DISPLAY_FIELDS: usize = 4;

    if tuple.len() <= MAX_DISPLAY_FIELDS {
        let items: Vec<String> = tuple.iter().map(|v| v.format_value(with_ty)).collect();
        format!("({})", items.join(", "))
    } else {
        let first_items: Vec<String> =
            tuple.iter().take(3).map(|v| v.format_value(with_ty)).collect();
        format!("({}, ...[{} fields])", first_items.join(", "), tuple.len())
    }
}
