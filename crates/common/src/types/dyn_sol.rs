use std::ops::{Deref, DerefMut};

use alloy_dyn_abi::DynSolValue;
use alloy_primitives::hex;
use alloy_primitives::{Address, FixedBytes, I256, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone)]
pub struct EdbSolValue(pub DynSolValue);

impl From<DynSolValue> for EdbSolValue {
    fn from(value: DynSolValue) -> Self {
        Self(value)
    }
}

impl From<EdbSolValue> for DynSolValue {
    fn from(value: EdbSolValue) -> Self {
        value.0
    }
}

impl Deref for EdbSolValue {
    type Target = DynSolValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EdbSolValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
enum SerializedDynSolValue {
    Bool(bool),
    Int { value: I256, bits: usize },
    Uint { value: U256, bits: usize },
    FixedBytes { value: FixedBytes<32>, size: usize },
    Address(Address),
    Function(FixedBytes<24>),
    Bytes(Vec<u8>),
    String(String),
    Array(Vec<SerializedDynSolValue>),
    FixedArray(Vec<SerializedDynSolValue>),
    Tuple(Vec<SerializedDynSolValue>),
    CustomStruct { name: String, prop_names: Vec<String>, tuple: Vec<SerializedDynSolValue> },
}

impl From<&DynSolValue> for SerializedDynSolValue {
    fn from(value: &DynSolValue) -> Self {
        match value {
            DynSolValue::Bool(b) => SerializedDynSolValue::Bool(*b),
            DynSolValue::Int(i, bits) => SerializedDynSolValue::Int { value: *i, bits: *bits },
            DynSolValue::Uint(u, bits) => SerializedDynSolValue::Uint { value: *u, bits: *bits },
            DynSolValue::FixedBytes(bytes, size) => {
                SerializedDynSolValue::FixedBytes { value: *bytes, size: *size }
            }
            DynSolValue::Address(addr) => SerializedDynSolValue::Address(*addr),
            DynSolValue::Function(func) => SerializedDynSolValue::Function(func.0),
            DynSolValue::Bytes(bytes) => SerializedDynSolValue::Bytes(bytes.clone()),
            DynSolValue::String(s) => SerializedDynSolValue::String(s.clone()),
            DynSolValue::Array(arr) => {
                SerializedDynSolValue::Array(arr.iter().map(Into::into).collect())
            }
            DynSolValue::FixedArray(arr) => {
                SerializedDynSolValue::FixedArray(arr.iter().map(Into::into).collect())
            }
            DynSolValue::Tuple(tuple) => {
                SerializedDynSolValue::Tuple(tuple.iter().map(Into::into).collect())
            }
            DynSolValue::CustomStruct { name, prop_names, tuple } => {
                SerializedDynSolValue::CustomStruct {
                    name: name.clone(),
                    prop_names: prop_names.clone(),
                    tuple: tuple.iter().map(Into::into).collect(),
                }
            }
        }
    }
}

impl From<SerializedDynSolValue> for DynSolValue {
    fn from(value: SerializedDynSolValue) -> Self {
        match value {
            SerializedDynSolValue::Bool(b) => DynSolValue::Bool(b),
            SerializedDynSolValue::Int { value, bits } => DynSolValue::Int(value, bits),
            SerializedDynSolValue::Uint { value, bits } => DynSolValue::Uint(value, bits),
            SerializedDynSolValue::FixedBytes { value, size } => {
                DynSolValue::FixedBytes(value, size)
            }
            SerializedDynSolValue::Address(addr) => DynSolValue::Address(addr),
            SerializedDynSolValue::Function(func) => {
                DynSolValue::Function(alloy_primitives::Function::from(func))
            }
            SerializedDynSolValue::Bytes(bytes) => DynSolValue::Bytes(bytes),
            SerializedDynSolValue::String(s) => DynSolValue::String(s),
            SerializedDynSolValue::Array(arr) => {
                DynSolValue::Array(arr.into_iter().map(Into::into).collect())
            }
            SerializedDynSolValue::FixedArray(arr) => {
                DynSolValue::FixedArray(arr.into_iter().map(Into::into).collect())
            }
            SerializedDynSolValue::Tuple(tuple) => {
                DynSolValue::Tuple(tuple.into_iter().map(Into::into).collect())
            }
            SerializedDynSolValue::CustomStruct { name, prop_names, tuple } => {
                DynSolValue::CustomStruct {
                    name,
                    prop_names,
                    tuple: tuple.into_iter().map(Into::into).collect(),
                }
            }
        }
    }
}

impl Serialize for EdbSolValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let serialized = SerializedDynSolValue::from(&self.0);
        serialized.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EdbSolValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let serialized = SerializedDynSolValue::deserialize(deserializer)?;
        Ok(EdbSolValue(serialized.into()))
    }
}

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
    fn format_value(&self, ctx: &SolValueFormatterContext) -> String;

    /// Returns the Solidity type of this value as a string.
    ///
    /// # Returns
    ///
    /// The Solidity type (e.g., "uint256", "address", "bytes32[]")
    fn format_type(&self) -> String;
}

#[derive(Default)]
pub struct SolValueFormatterContext {
    pub resolve_address: Option<Box<dyn Fn(Address) -> Option<String>>>,
    pub with_ty: bool,
    pub shorten_long: bool,
}

impl SolValueFormatterContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_ty() -> Self {
        Self { with_ty: true, ..Default::default() }
    }
}

impl SolValueFormatter for DynSolValue {
    fn format_value(&self, ctx: &SolValueFormatterContext) -> String {
        let value_str = match self {
            DynSolValue::Bool(b) => b.to_string(),

            DynSolValue::Int(n, bits) => {
                if ctx.with_ty {
                    format!("int{}({})", bits, n)
                } else {
                    n.to_string()
                }
            }

            DynSolValue::Uint(n, bits) => {
                if ctx.with_ty {
                    format!("uint{}({})", bits, n)
                } else {
                    n.to_string()
                }
            }

            DynSolValue::Address(addr) => {
                if let Some(label) = ctx.resolve_address.as_ref().and_then(|f| f(*addr)) {
                    label
                } else {
                    let addr_str = if !ctx.shorten_long {
                        format!("0x{:040x}", addr)
                    } else if *addr == Address::ZERO {
                        "0x0000000000000000".to_string()
                    } else {
                        let addr_str = format!("{:?}", addr);
                        // Show more characters for better identification: 8 chars + ... + 6 chars
                        format!("{}...{}", &addr_str[..8], &addr_str[addr_str.len() - 6..])
                    };
                    if ctx.with_ty {
                        format!("address({})", addr_str)
                    } else {
                        addr_str
                    }
                }
            }

            DynSolValue::Function(func) => {
                format!("0x{}", hex::encode(func.as_slice()))
            }

            DynSolValue::FixedBytes(bytes, size) => {
                if ctx.with_ty {
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

            DynSolValue::Array(arr) => format_array(arr, false, ctx),

            DynSolValue::FixedArray(arr) => format_array(arr, true, ctx),

            DynSolValue::Tuple(tuple) => format_tuple(tuple, ctx),

            DynSolValue::CustomStruct { name, prop_names, tuple } => {
                if prop_names.is_empty() {
                    format!("{}{}", name, format_tuple(tuple, ctx))
                } else {
                    let fields: Vec<String> = tuple
                        .iter()
                        .zip(prop_names.iter())
                        .map(|(value, name)| format!("{}: {}", name, value.format_value(ctx)))
                        .collect();

                    if ctx.with_ty {
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

fn format_array(arr: &[DynSolValue], is_fixed: bool, ctx: &SolValueFormatterContext) -> String {
    const MAX_DISPLAY_ITEMS: usize = 5;

    if arr.is_empty() {
        return "[]".to_string();
    }

    if arr.len() <= MAX_DISPLAY_ITEMS || !ctx.shorten_long {
        let items: Vec<String> = arr.iter().map(|v| v.format_value(ctx)).collect();
        format!("[{}]", items.join(", "))
    } else {
        let first_items: Vec<String> = arr.iter().take(3).map(|v| v.format_value(ctx)).collect();

        let suffix = if is_fixed {
            format!(", ...[{} total]", arr.len())
        } else {
            format!(", ...[{} items]", arr.len())
        };

        format!("[{}{}]", first_items.join(", "), suffix)
    }
}

fn format_tuple(tuple: &[DynSolValue], ctx: &SolValueFormatterContext) -> String {
    if tuple.is_empty() {
        return "()".to_string();
    }

    if tuple.len() == 1 {
        return format!("({})", tuple[0].format_value(ctx));
    }

    const MAX_DISPLAY_FIELDS: usize = 4;

    if tuple.len() <= MAX_DISPLAY_FIELDS || !ctx.shorten_long {
        let items: Vec<String> = tuple.iter().map(|v| v.format_value(ctx)).collect();
        format!("({})", items.join(", "))
    } else {
        let first_items: Vec<String> = tuple.iter().take(3).map(|v| v.format_value(ctx)).collect();
        format!("({}, ...[{} fields])", first_items.join(", "), tuple.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, bytes, uint, Address, FixedBytes, I256, U256};
    use serde_json;

    #[test]
    fn test_serialize_deserialize_bool() {
        let value = EdbSolValue(DynSolValue::Bool(true));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Bool(b) => assert!(b),
            _ => panic!("Expected Bool variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_uint() {
        let value = EdbSolValue(DynSolValue::Uint(U256::from(42u64), 256));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Uint(u, bits) => {
                assert_eq!(u, U256::from(42u64));
                assert_eq!(bits, 256);
            }
            _ => panic!("Expected Uint variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_int() {
        let value = EdbSolValue(DynSolValue::Int(I256::try_from(-42i64).unwrap(), 256));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Int(i, bits) => {
                assert_eq!(i, I256::try_from(-42i64).unwrap());
                assert_eq!(bits, 256);
            }
            _ => panic!("Expected Int variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_address() {
        let addr = address!("0000000000000000000000000000000000000001");
        let value = EdbSolValue(DynSolValue::Address(addr));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Address(a) => assert_eq!(a, addr),
            _ => panic!("Expected Address variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_bytes() {
        let value = EdbSolValue(DynSolValue::Bytes(vec![1, 2, 3, 4]));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Bytes(b) => assert_eq!(b, vec![1, 2, 3, 4]),
            _ => panic!("Expected Bytes variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_fixed_bytes() {
        let bytes = FixedBytes::<32>::from([1u8; 32]);
        let value = EdbSolValue(DynSolValue::FixedBytes(bytes, 32));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::FixedBytes(b, size) => {
                assert_eq!(b, bytes);
                assert_eq!(size, 32);
            }
            _ => panic!("Expected FixedBytes variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_string() {
        let value = EdbSolValue(DynSolValue::String("Hello, Ethereum!".to_string()));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::String(s) => assert_eq!(s, "Hello, Ethereum!"),
            _ => panic!("Expected String variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_array() {
        let value = EdbSolValue(DynSolValue::Array(vec![
            DynSolValue::Uint(U256::from(1u64), 256),
            DynSolValue::Uint(U256::from(2u64), 256),
            DynSolValue::Uint(U256::from(3u64), 256),
        ]));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                match &arr[0] {
                    DynSolValue::Uint(u, _) => assert_eq!(*u, U256::from(1u64)),
                    _ => panic!("Expected Uint in array"),
                }
            }
            _ => panic!("Expected Array variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_fixed_array() {
        let value = EdbSolValue(DynSolValue::FixedArray(vec![
            DynSolValue::Bool(true),
            DynSolValue::Bool(false),
        ]));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::FixedArray(arr) => {
                assert_eq!(arr.len(), 2);
                match (&arr[0], &arr[1]) {
                    (DynSolValue::Bool(b1), DynSolValue::Bool(b2)) => {
                        assert!(*b1);
                        assert!(!*b2);
                    }
                    _ => panic!("Expected Bool values in fixed array"),
                }
            }
            _ => panic!("Expected FixedArray variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_tuple() {
        let addr = address!("0000000000000000000000000000000000000001");
        let value = EdbSolValue(DynSolValue::Tuple(vec![
            DynSolValue::Address(addr),
            DynSolValue::Uint(U256::from(100u64), 256),
            DynSolValue::String("test".to_string()),
        ]));
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Tuple(tuple) => {
                assert_eq!(tuple.len(), 3);
                match (&tuple[0], &tuple[1], &tuple[2]) {
                    (DynSolValue::Address(a), DynSolValue::Uint(u, _), DynSolValue::String(s)) => {
                        assert_eq!(*a, addr);
                        assert_eq!(*u, U256::from(100u64));
                        assert_eq!(s, "test");
                    }
                    _ => panic!("Expected (Address, Uint, String) in tuple"),
                }
            }
            _ => panic!("Expected Tuple variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_nested_structure() {
        let value = EdbSolValue(DynSolValue::Array(vec![
            DynSolValue::Tuple(vec![
                DynSolValue::Uint(U256::from(1u64), 256),
                DynSolValue::Array(vec![
                    DynSolValue::String("nested1".to_string()),
                    DynSolValue::String("nested2".to_string()),
                ]),
            ]),
            DynSolValue::Tuple(vec![
                DynSolValue::Uint(U256::from(2u64), 256),
                DynSolValue::Array(vec![DynSolValue::String("nested3".to_string())]),
            ]),
        ]));

        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::Array(arr) => {
                assert_eq!(arr.len(), 2);
                match &arr[0] {
                    DynSolValue::Tuple(tuple) => {
                        assert_eq!(tuple.len(), 2);
                        match &tuple[1] {
                            DynSolValue::Array(inner_arr) => {
                                assert_eq!(inner_arr.len(), 2);
                            }
                            _ => panic!("Expected Array in tuple"),
                        }
                    }
                    _ => panic!("Expected Tuple in array"),
                }
            }
            _ => panic!("Expected Array variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_custom_struct() {
        let value = EdbSolValue(DynSolValue::CustomStruct {
            name: "Person".to_string(),
            prop_names: vec!["name".to_string(), "age".to_string()],
            tuple: vec![
                DynSolValue::String("Alice".to_string()),
                DynSolValue::Uint(U256::from(30u64), 256),
            ],
        });

        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: EdbSolValue = serde_json::from_str(&serialized).unwrap();

        match deserialized.0 {
            DynSolValue::CustomStruct { name, prop_names, tuple } => {
                assert_eq!(name, "Person");
                assert_eq!(prop_names, vec!["name", "age"]);
                assert_eq!(tuple.len(), 2);
                match (&tuple[0], &tuple[1]) {
                    (DynSolValue::String(s), DynSolValue::Uint(u, _)) => {
                        assert_eq!(s, "Alice");
                        assert_eq!(*u, U256::from(30u64));
                    }
                    _ => panic!("Expected (String, Uint) in custom struct"),
                }
            }
            _ => panic!("Expected CustomStruct variant"),
        }
    }

    #[test]
    fn test_json_format_readability() {
        let value = EdbSolValue(DynSolValue::Tuple(vec![
            DynSolValue::Bool(true),
            DynSolValue::Uint(U256::from(42u64), 256),
        ]));

        let json = serde_json::to_string_pretty(&value).unwrap();
        // Verify that the JSON is readable and contains expected structure
        assert!(json.contains("\"type\""));
        assert!(json.contains("\"value\""));
        assert!(json.contains("\"Tuple\""));
    }
}
