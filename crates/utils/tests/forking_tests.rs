use edb_utils::{fork_and_prepare, get_tx_env_from_tx, ForkInfo};
use alloy_primitives::{address, b256, TxHash, U256};
use alloy_rpc_types::Transaction;
use revm::primitives::hardfork::SpecId;

/// Test transaction hash from a known mainnet transaction
/// This is the first USDC transfer transaction: https://etherscan.io/tx/0x523d98ca8f46fcd32517f30f62704c55dc4de5dccbbec7bbf7e11b7116f00eca
const TEST_TX_HASH: &str = "0x523d98ca8f46fcd32517f30f62704c55dc4de5dccbbec7bbf7e11b7116f00eca";

/// Another test transaction for testing multiple transactions in a block
/// Random USDT transfer: https://etherscan.io/tx/0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060
const TEST_TX_HASH_2: &str = "0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060";

#[tokio::test]
async fn test_fork_info_creation() {
    let fork_info = ForkInfo {
        block_number: 12345678,
        block_hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
        timestamp: 1640995200,
        chain_id: 1,
        spec_id: SpecId::LONDON,
    };

    assert_eq!(fork_info.block_number, 12345678);
    assert_eq!(fork_info.chain_id, 1);
    assert_eq!(fork_info.spec_id, SpecId::LONDON);
}

#[tokio::test]
async fn test_fork_and_prepare_with_real_transaction() {
    // Use a public RPC endpoint for testing (consider using env var in production)
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());
    
    let tx_hash: TxHash = TEST_TX_HASH.parse().expect("valid tx hash");
    
    let result = fork_and_prepare(&rpc_url, tx_hash).await;
    
    match result {
        Ok(fork_result) => {
            // Verify fork info
            assert_eq!(fork_result.fork_info.chain_id, 1, "Should be mainnet");
            assert!(fork_result.fork_info.block_number > 0, "Block number should be positive");
            
            // The test transaction is from block 4,060,175
            assert_eq!(fork_result.fork_info.block_number, 4060175, "Should match known block");
            
            // Verify we have a valid context
            // The context should contain the forked state
            
            // Verify target transaction environment
            assert_eq!(
                fork_result.target_tx_env.chain_id, 
                Some(1), 
                "Target tx should have mainnet chain ID"
            );
        }
        Err(e) => {
            panic!("Failed to fork and prepare: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fork_with_multiple_preceding_transactions() {
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());
    
    // Use a transaction that's not the first in its block
    let tx_hash: TxHash = TEST_TX_HASH_2.parse().expect("valid tx hash");
    
    let result = fork_and_prepare(&rpc_url, tx_hash).await;
    
    match result {
        Ok(fork_result) => {
            // This transaction is in block 10,207,858
            assert_eq!(fork_result.fork_info.block_number, 10207858);
            
            // The spec ID for this block should be Berlin (block 12,244,000 is after this)
            assert_eq!(fork_result.fork_info.spec_id, SpecId::BERLIN);
            
            println!("Successfully forked at block {} with spec {:?}", 
                fork_result.fork_info.block_number,
                fork_result.fork_info.spec_id
            );
        }
        Err(e) => {
            panic!("Failed to fork and prepare: {:?}", e);
        }
    }
}

#[test]
fn test_get_tx_env_from_tx() {
    // Create a mock transaction for testing
    let raw_tx = r#"{
        "blockHash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "blockNumber": "0xf4240",
        "from": "0x742d35cc6634c0532925a3b844bc9e7d62a7e6e5",
        "gas": "0x5208",
        "gasPrice": "0x3b9aca00",
        "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "input": "0x",
        "nonce": "0x0",
        "to": "0x7be8076f4ea4a4ad08075c2508e481d6c946d12b",
        "transactionIndex": "0x0",
        "value": "0xde0b6b3a7640000",
        "type": "0x0",
        "chainId": "0x1",
        "v": "0x1b",
        "r": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "s": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
    }"#;
    
    let tx: Transaction = serde_json::from_str(raw_tx).expect("valid transaction JSON");
    let chain_id = 1u64;
    
    let result = get_tx_env_from_tx(&tx, chain_id);
    
    match result {
        Ok(tx_env) => {
            assert_eq!(tx_env.chain_id, Some(chain_id));
            assert_eq!(tx_env.nonce, 0);
            assert_eq!(tx_env.gas_limit, 0x5208); // 21000 in hex
            assert_eq!(tx_env.value, U256::from(0xde0b6b3a7640000u64)); // 1 ETH in wei
            assert!(tx_env.data.is_empty());
        }
        Err(e) => {
            panic!("Failed to convert transaction to TxEnv: {:?}", e);
        }
    }
}

#[test]
fn test_get_tx_env_contract_creation() {
    // Test with a contract creation transaction (no "to" field)
    let raw_tx = r#"{
        "blockHash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "blockNumber": "0xf4240",
        "from": "0x742d35cc6634c0532925a3b844bc9e7d62a7e6e5",
        "gas": "0x186a0",
        "gasPrice": "0x3b9aca00",
        "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "input": "0x606060405260008054600160a060020a03191633179055",
        "nonce": "0x1",
        "to": null,
        "transactionIndex": "0x0",
        "value": "0x0",
        "type": "0x0",
        "chainId": "0x1",
        "v": "0x1b",
        "r": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "s": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
    }"#;
    
    let tx: Transaction = serde_json::from_str(raw_tx).expect("valid transaction JSON");
    let chain_id = 1u64;
    
    let result = get_tx_env_from_tx(&tx, chain_id);
    
    match result {
        Ok(tx_env) => {
            // Verify it's a contract creation
            assert!(matches!(tx_env.kind, alloy_primitives::TxKind::Create));
            assert_eq!(tx_env.nonce, 1);
            assert_eq!(tx_env.gas_limit, 0x186a0); // 100000 in hex
            assert_eq!(tx_env.value, U256::ZERO);
            assert!(!tx_env.data.is_empty()); // Contract bytecode
        }
        Err(e) => {
            panic!("Failed to convert contract creation tx to TxEnv: {:?}", e);
        }
    }
}

#[test]
fn test_get_tx_env_with_access_list() {
    // Test with EIP-2930 transaction that includes access list
    let raw_tx = r#"{
        "blockHash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "blockNumber": "0xf4240",
        "from": "0x742d35cc6634c0532925a3b844bc9e7d62a7e6e5",
        "gas": "0x5208",
        "gasPrice": "0x3b9aca00",
        "maxFeePerGas": "0x77359400",
        "maxPriorityFeePerGas": "0x59682f00",
        "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "input": "0x",
        "nonce": "0x2",
        "to": "0x7be8076f4ea4a4ad08075c2508e481d6c946d12b",
        "transactionIndex": "0x0",
        "value": "0x0",
        "type": "0x2",
        "accessList": [
            {
                "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "storageKeys": [
                    "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "0x0000000000000000000000000000000000000000000000000000000000000001"
                ]
            }
        ],
        "chainId": "0x1",
        "v": "0x1",
        "r": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "s": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
    }"#;
    
    let tx: Transaction = serde_json::from_str(raw_tx).expect("valid transaction JSON");
    let chain_id = 1u64;
    
    let result = get_tx_env_from_tx(&tx, chain_id);
    
    match result {
        Ok(tx_env) => {
            assert_eq!(tx_env.chain_id, Some(chain_id));
            assert_eq!(tx_env.nonce, 2);
            
            // Check that access list is properly converted
            let access_list = tx_env.access_list;
            assert_eq!(access_list.len(), 1);
            assert_eq!(
                access_list[0].address,
                address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
            );
            assert_eq!(access_list[0].storage_keys.len(), 2);
        }
        Err(e) => {
            panic!("Failed to convert EIP-2930 tx to TxEnv: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fork_at_specific_hardfork_boundaries() {
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());
    
    // Test transactions at different hardfork boundaries
    struct HardforkTest {
        tx_hash: &'static str,
        expected_block: u64,
        expected_spec: SpecId,
        description: &'static str,
    }
    
    let tests = vec![
        HardforkTest {
            // Transaction from Homestead era
            tx_hash: "0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060",
            expected_block: 46147,
            expected_spec: SpecId::FRONTIER,
            description: "Frontier era transaction",
        },
        // Add more test cases for different eras as needed
    ];
    
    for test in tests {
        println!("Testing: {}", test.description);
        let tx_hash: TxHash = test.tx_hash.parse().expect("valid tx hash");
        
        match fork_and_prepare(&rpc_url, tx_hash).await {
            Ok(fork_result) => {
                assert_eq!(
                    fork_result.fork_info.block_number, 
                    test.expected_block,
                    "{}: Wrong block number", 
                    test.description
                );
                assert_eq!(
                    fork_result.fork_info.spec_id, 
                    test.expected_spec,
                    "{}: Wrong spec ID", 
                    test.description
                );
            }
            Err(e) => {
                panic!("{} failed: {:?}", test.description, e);
            }
        }
    }
}