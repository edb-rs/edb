use std::{collections::HashMap, path::PathBuf};

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Code {
    Opcode(OpcodeInfo),
    Source(SourceInfo),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpcodeInfo {
    pub address: Address,
    pub bytecode_address: Address,
    pub codes: HashMap<u64, String>, // pc -> opcode
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceInfo {
    pub address: Address,
    pub bytecode_address: Address,
    pub sources: HashMap<PathBuf, String>, // file -> source
}
