use foundry_compilers::artifacts::{Mutability, TypeName};

use crate::{
    analysis::{VariableRef, USID, UVID},
    HOOK_TRIGGER_ADDRESS, VARIABLE_UPDATE_ADDRESS,
};

static EDB_STATE_VAR_FLAG: &str = "_edb_state_var_";

pub fn generate_step_hook(usid: USID) -> String {
    format!("address({:?}).call(hex\"{:064x}\");", HOOK_TRIGGER_ADDRESS, u64::from(usid),)
}

/// Generates a variable update hook.
pub fn generate_variable_update_hook(uvid: UVID, variable: &VariableRef) -> String {
    let base_var = variable.base();
    let base_name = &base_var.declaration().name;
    format!(
        "require(keccak256(abi.encode({}, {}, {})) != bytes32(uint256(0x2333)));",
        VARIABLE_UPDATE_ADDRESS,
        u64::from(uvid),
        base_name,
    )
}

/// Generates a view method for a state variable.
///
/// For primitive types, generates a simple view function.
/// For arrays and mappings, recursively adds index/key parameters.
/// Returns None if the variable contains user-defined types or is constant.
///
/// # Arguments
/// * `private_state_variable` - The VariableRef for the private state variable
///
/// # Returns
/// * `Option<String>` - The generated view function code, or None if user-defined types are present
pub fn generate_view_method(state_variable: &VariableRef) -> Option<String> {
    let declaration = state_variable.declaration();
    if declaration.mutability == Some(Mutability::Constant) {
        // We do not need to output constant state variables
        return None;
    }

    let var_name = &declaration.name;

    // Get the type information
    let type_name = declaration.type_name.as_ref()?;

    // Check if the type contains user-defined types and get parameter info
    let (params, return_type) = analyze_type_for_view_method(type_name)?;

    // Generate the function signature
    let params_str = if params.is_empty() { String::new() } else { params.join(", ") };

    // Generate the function body
    let body = generate_view_body(var_name, &params);

    // Get UVID
    let uvid = state_variable.id();

    // Construct the complete function with EDB prefix
    Some(format!(
        "    function {}{}{}({}) public view returns ({}) {{\n        return {};\n    }}",
        var_name, EDB_STATE_VAR_FLAG, uvid, params_str, return_type, body
    ))
}

/// Analyzes a TypeName and returns parameter list and return type for the view function.
/// Returns None if user-defined types are found.
fn analyze_type_for_view_method(type_name: &TypeName) -> Option<(Vec<String>, String)> {
    analyze_type_recursive(type_name, 0)
}

/// Recursively analyzes a TypeName to build parameter list and return type.
/// Returns None if user-defined types are found.
fn analyze_type_recursive(type_name: &TypeName, depth: usize) -> Option<(Vec<String>, String)> {
    match type_name {
        TypeName::ElementaryTypeName(elementary) => {
            // Elementary types are primitive types like uint, address, bool, etc.
            let return_type = format_return_type(&elementary.name);
            Some((Vec::new(), return_type))
        }
        TypeName::Mapping(mapping) => {
            // For mapping, we need to add a key parameter and recurse on value type
            let key_type = &mapping.key_type;
            let value_type = &mapping.value_type;

            // Get key type as string (must be elementary)
            let key_type_str = match key_type {
                TypeName::ElementaryTypeName(elem) => elem.name.clone(),
                _ => return None, // Mapping keys must be elementary types
            };

            // Recurse on value type
            let (mut sub_params, return_type) = analyze_type_recursive(value_type, depth + 1)?;

            // Add key parameter at the beginning
            let param_name = if depth == 0 { "key".to_string() } else { format!("key{}", depth) };
            let key_param = format!("{} {}", key_type_str, param_name);

            let mut params = vec![key_param];
            params.append(&mut sub_params);

            Some((params, return_type))
        }
        TypeName::ArrayTypeName(array) => {
            // For arrays, we need to add an index parameter and recurse on base type
            let base_type = &array.base_type;

            // Recurse on base type
            let (mut sub_params, return_type) = analyze_type_recursive(base_type, depth + 1)?;

            // Add index parameter at the beginning
            let param_name =
                if depth == 0 { "index".to_string() } else { format!("index{}", depth) };
            let index_param = format!("uint256 {}", param_name);

            let mut params = vec![index_param];
            params.append(&mut sub_params);

            Some((params, return_type))
        }
        TypeName::UserDefinedTypeName(_) => {
            // User-defined types (structs, enums, contracts) - skip
            None
        }
        TypeName::FunctionTypeName(_) => {
            // Function types - skip
            None
        }
    }
}

/// Generates the body of the view function based on the variable name and parameters.
fn generate_view_body(var_name: &str, params: &[String]) -> String {
    if params.is_empty() {
        var_name.to_string()
    } else {
        // Extract parameter names from the parameter declarations
        let param_names: Vec<String> = params
            .iter()
            .map(|p| {
                // Extract the parameter name (last word in the parameter declaration)
                p.split_whitespace().last().unwrap_or("").to_string()
            })
            .collect();

        // Build the access expression (e.g., "myVar[key][index]")
        let mut body = var_name.to_string();
        for param_name in param_names {
            body = format!("{}[{}]", body, param_name);
        }
        body
    }
}

/// Formats the return type with appropriate data location for reference types.
/// For reference types (string, arrays, structs), adds "memory" data location.
/// For value types, returns as-is.
fn format_return_type(type_name: &str) -> String {
    match type_name {
        // Reference types that need memory data location
        t if t.starts_with("string") => format!("{} memory", t),
        // Dynamic arrays (ends with [])
        t if t.ends_with("[]") => format!("{} memory", t),
        // Fixed-size arrays (contains [n] where n is a number)
        t if t.contains('[') && t.contains(']') => format!("{} memory", t),
        // Value types don't need data location
        _ => type_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis;

    #[test]
    fn test_generate_view_method_primitive_types() {
        let source = r#"
        contract C {
            uint256 private myValue;
            address private owner;
            bool private isActive;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        // Should find 3 private state variables
        assert_eq!(analysis.private_state_variables.len(), 3);

        // Test each private state variable
        for private_var in &analysis.private_state_variables {
            let result = super::generate_view_method(private_var);
            assert!(
                result.is_some(),
                "Should generate view method for primitive type: {}",
                private_var.declaration().name
            );

            let code = result.unwrap();
            let var_name = &private_var.declaration().name;

            // Check function signature contains the variable name with EDB suffix
            assert!(code.contains(&format!("function {}_edb_state_var_", var_name)));
            assert!(code.contains("public view returns"));

            // Check function body returns the variable
            assert!(code.contains(&format!("return {};", var_name)));
        }
    }

    #[test]
    fn test_generate_view_method_mapping_types() {
        let source = r#"
        contract C {
            mapping(address => uint256) private balances;
            mapping(uint256 => bool) private permissions;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        // Should find 2 private state variables
        assert_eq!(analysis.private_state_variables.len(), 2);

        for private_var in &analysis.private_state_variables {
            let result = super::generate_view_method(private_var);
            assert!(
                result.is_some(),
                "Should generate view method for mapping: {}",
                private_var.declaration().name
            );

            let code = result.unwrap();
            let var_name = &private_var.declaration().name;

            // Check function signature has a key parameter with EDB suffix
            assert!(code.contains(&format!("function {}_edb_state_var_", var_name)));
            assert!(code.contains("key) public view returns"));

            // Check function body accesses the mapping with key
            assert!(code.contains(&format!("return {}[key];", var_name)));
        }
    }

    #[test]
    fn test_generate_view_method_array_types() {
        let source = r#"
        contract C {
            uint256[] private numbers;
            address[] private addresses;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        // Should find 2 private state variables
        assert_eq!(analysis.private_state_variables.len(), 2);

        for private_var in &analysis.private_state_variables {
            let result = super::generate_view_method(private_var);
            assert!(
                result.is_some(),
                "Should generate view method for array: {}",
                private_var.declaration().name
            );

            let code = result.unwrap();
            let var_name = &private_var.declaration().name;

            // Check function signature has an index parameter with EDB suffix
            assert!(code.contains(&format!("function {}_edb_state_var_", var_name)));
            assert!(code.contains("(uint256 index)"));
            assert!(code.contains("public view returns"));

            // Check function body accesses the array with index
            assert!(code.contains(&format!("return {}[index];", var_name)));
        }
    }

    #[test]
    fn test_generate_view_method_nested_types() {
        let source = r#"
        contract C {
            mapping(address => uint256[]) private userTokens;
            mapping(uint256 => mapping(address => bool)) private permissions;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        // Should find 2 private state variables
        assert_eq!(analysis.private_state_variables.len(), 2);

        let user_tokens =
            analysis.private_state_variables.iter().find(|v| v.declaration().name == "userTokens");
        let permissions =
            analysis.private_state_variables.iter().find(|v| v.declaration().name == "permissions");

        // Test userTokens mapping(address => uint256[])
        if let Some(user_tokens_var) = user_tokens {
            let result = super::generate_view_method(user_tokens_var);
            assert!(result.is_some(), "Should generate view method for nested mapping->array");

            let code = result.unwrap();
            // Should have address key parameter and uint256 index1 parameter (depth-based naming) with EDB suffix
            assert!(code.contains("function userTokens_edb_state_var_"));
            assert!(code.contains("(address key, uint256 index1)"));
            assert!(code.contains("return userTokens[key][index1];"));
        }

        // Test permissions mapping(uint256 => mapping(address => bool))
        if let Some(permissions_var) = permissions {
            let result = super::generate_view_method(permissions_var);
            assert!(result.is_some(), "Should generate view method for nested mapping->mapping");

            let code = result.unwrap();
            // Should have uint256 key parameter and address key1 parameter (depth-based naming) with EDB suffix
            assert!(code.contains("function permissions_edb_state_var_"));
            assert!(code.contains("(uint256 key, address key1)"));
            assert!(code.contains("return permissions[key][key1];"));
        }
    }

    #[test]
    fn test_generate_view_method_user_defined_types() {
        let source = r#"
        contract C {
            struct User {
                uint256 balance;
                address addr;
            }
            
            User private userData;
            User[] private users;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        // Note: If no private state variables with user-defined types are detected,
        // this test verifies that the analysis correctly filters them out.
        // If they are detected, they should return None from generate_view_method

        for private_var in &analysis.private_state_variables {
            let result = super::generate_view_method(private_var);
            // Should return None for user-defined types
            assert!(
                result.is_none(),
                "Should not generate view method for user-defined type: {}",
                private_var.declaration().name
            );
        }
    }

    #[test]
    fn test_generate_view_method_reference_types() {
        let source = r#"
        contract C {
            string private message;
            string[] private messages;
        }
        "#;
        let (_sources, analysis) = analysis::tests::compile_and_analyze(source);

        // Should find 2 private state variables
        assert_eq!(analysis.private_state_variables.len(), 2);

        for private_var in &analysis.private_state_variables {
            let result = super::generate_view_method(private_var);
            assert!(
                result.is_some(),
                "Should generate view method for reference type: {}",
                private_var.declaration().name
            );

            let code = result.unwrap();
            let var_name = &private_var.declaration().name;

            // Check that memory data location is added for reference types
            match var_name.as_str() {
                "message" => {
                    assert!(code.contains("function message_edb_state_var_"));
                    assert!(code.contains("() public view returns (string memory)"));
                    assert!(code.contains("return message;"));
                }
                "messages" => {
                    assert!(code.contains("function messages_edb_state_var_"));
                    assert!(code.contains("(uint256 index) public view returns (string memory)"));
                    assert!(code.contains("return messages[index];"));
                }
                _ => panic!("Unexpected variable name: {}", var_name),
            }
        }
    }
}
