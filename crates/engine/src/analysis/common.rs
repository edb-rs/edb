use alloy_primitives::{Address, TxHash};
use std::collections::HashMap;

/// Main analysis result containing debugging information from source code analysis
/// This will be populated by the analysis module in the future
#[derive(Debug)]
pub struct AnalysisResult {
    /// Transaction hash that was analyzed
    pub tx_hash: TxHash,
    /// All contract addresses touched during execution
    pub touched_contracts: Vec<Address>,
    /// Map of contract addresses to their source code
    pub source_code: HashMap<Address, String>,
    // TODO: Add more analysis results from instrumentation and source analysis
}
