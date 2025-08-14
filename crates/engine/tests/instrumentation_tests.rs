use alloy_primitives::Address;
use edb_engine::instrumentation::{instrument_sources, INSTRUMENTATION_PRECOMPILE};
use std::collections::HashMap;
use tracing::{debug, info, warn};

#[test]
fn test_simple_function_instrumentation() {
    edb_utils::logging::ensure_test_logging(None);
    info!("Running test");
    let source = r#"
pragma solidity ^0.8.0;

contract SimpleContract {
    uint256 public value;
    
    function setValue(uint256 _value) public {
        value = _value;
    }
    
    function getValue() public view returns (uint256) {
        return value;
    }
}
"#;

    let mut sources = HashMap::new();
    let addr = Address::ZERO;
    sources.insert(addr, source.to_string());

    let result = instrument_sources(&sources).unwrap();
    let instrumented = result.get(&addr).unwrap();

    // Check that instrumentation was added
    assert!(instrumented.contains("assembly"));
    assert!(
        instrumented.contains(&format!("0x{}", hex::encode(INSTRUMENTATION_PRECOMPILE.as_slice())))
    );

    // Check that functions are still present
    assert!(instrumented.contains("function setValue"));
    assert!(instrumented.contains("function getValue"));
}

#[test]
fn test_multiple_function_instrumentation() {
    edb_utils::logging::ensure_test_logging(None);
    info!("Running test");
    let source = r#"
contract MultiFunction {
    mapping(address => uint256) public balances;
    
    function deposit() public payable {
        balances[msg.sender] += msg.value;
    }
    
    function withdraw(uint256 amount) public {
        require(balances[msg.sender] >= amount, "Insufficient balance");
        balances[msg.sender] -= amount;
        payable(msg.sender).transfer(amount);
    }
    
    function getBalance(address account) public view returns (uint256) {
        return balances[account];
    }
}
"#;

    let mut sources = HashMap::new();
    let addr = Address::ZERO;
    sources.insert(addr, source.to_string());

    let result = instrument_sources(&sources).unwrap();
    let instrumented = result.get(&addr).unwrap();

    // Count occurrences of assembly blocks (should be one per function)
    let assembly_count = instrumented.matches("assembly").count();
    assert!(assembly_count >= 2, "Expected at least 2 assembly blocks, got {}", assembly_count);
}

#[test]
fn test_empty_source_handling() {
    edb_utils::logging::ensure_test_logging(None);
    info!("Running test");
    let mut sources = HashMap::new();
    let addr = Address::ZERO;
    sources.insert(addr, "".to_string());

    let result = instrument_sources(&sources);
    assert!(result.is_ok());
}

#[test]
fn test_contract_without_functions() {
    edb_utils::logging::ensure_test_logging(None);
    info!("Running test");
    let source = r#"
pragma solidity ^0.8.0;

contract NoFunctions {
    uint256 public constant VALUE = 42;
}
"#;

    let mut sources = HashMap::new();
    let addr = Address::ZERO;
    sources.insert(addr, source.to_string());

    let result = instrument_sources(&sources).unwrap();
    let instrumented = result.get(&addr).unwrap();

    // Should not add any instrumentation
    assert!(!instrumented.contains("assembly"));
}
