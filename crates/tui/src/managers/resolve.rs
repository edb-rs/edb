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

//! Information management for TUI panels
//!
//! This module implements a two-tier architecture for info management:
//!
//! - `InfoManager`: Per-thread instance for immediate data access during rendering
//! - `InfoManagerCore`: Shared core that handles RPC communication and data fetching
//!
//! This design ensures rendering threads never block on network I/O while
//! maintaining consistency across the application.

use crate::{managers::FetchCache, rpc::RpcClient};
use alloy_dyn_abi::{DynSolValue, EventExt, FunctionExt, JsonAbiExt};
use alloy_json_abi::JsonAbi;
use alloy_primitives::{hex, Address, Bytes, LogData, Selector, U256};
use edb_common::SolValueFormatter;
use eyre::Result;
use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PendingRequest {
    /// Request for contract ABI
    ContractAbi(Address, bool),

    /// Request for contract constructor arguments
    ConstructorArgs(Address),
}

/// Per-thread info manager providing cached data for rendering
///
/// # Design Philosophy
///
/// `Resolver` follows the same pattern as ThemeManager:
/// - All data reads are immediate and non-blocking
/// - Complex RPC operations are delegated to InfoManagerCore
/// - Data synchronization happens explicitly via `fetch_data()`
///
/// # Usage Pattern
///
/// ```ignore
/// // When data updates are needed
/// resolver.fetch_data().await?; // Sync with core
///
/// // During rendering - immediate access to cached data
/// // (future: add cached fields here for immediate reads)
/// ```
#[derive(Debug, Clone)]
pub struct Resolver {
    /// Pending requests
    pending_requests: HashSet<PendingRequest>,

    contract_abi: FetchCache<(Address, bool), JsonAbi>,
    constructor_args: FetchCache<Address, Bytes>,

    core: Arc<RwLock<ResolverCore>>,
}

impl Deref for Resolver {
    type Target = Arc<RwLock<ResolverCore>>;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for Resolver {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl Resolver {
    /// Create a new info manager with a shared core
    pub fn new(core: Arc<RwLock<ResolverCore>>) -> Self {
        Self {
            pending_requests: HashSet::new(),
            contract_abi: FetchCache::new(),
            constructor_args: FetchCache::new(),
            core,
        }
    }

    /// Add a new fetching request
    fn new_fetching_request(&mut self, request: PendingRequest) {
        self.pending_requests.insert(request);
    }

    /// Fetch the constructor arguments for a specific address
    pub fn get_constructor_args(&mut self, address: Address) -> Option<Bytes> {
        match self.constructor_args.get(&address) {
            Some(args) => args.clone(),
            _ => {
                debug!("Constructor arguments not found in cache, fetching...");
                self.new_fetching_request(PendingRequest::ConstructorArgs(address));
                None
            }
        }
    }

    /// Fetch the contract ABI for a specific address
    pub fn get_contract_abi(&mut self, address: Address, recompiled: bool) -> Option<JsonAbi> {
        match self.contract_abi.get(&(address, recompiled)) {
            Some(abi) => abi.clone(),
            _ => {
                debug!("Contract ABI not found in cache, fetching...");
                self.new_fetching_request(PendingRequest::ContractAbi(address, recompiled));
                None
            }
        }
    }

    /// Resolve function return
    pub fn resolve_function_return(
        &mut self,
        calldata: &Bytes,
        output: &Bytes,
        address: Option<Address>,
    ) -> Option<String> {
        if calldata.len() < 4 {
            return None;
        }

        let selector = Selector::from_slice(&calldata[..4]);
        if let Some(function_abi) = address
            .and_then(|addr| self.get_contract_abi(addr, false))
            .and_then(|abi| abi.function_by_selector(selector).cloned())
        {
            match function_abi.abi_decode_output(output) {
                Ok(decoded_values) => {
                    if decoded_values.is_empty() {
                        return Some("()".to_string());
                    }

                    // Format decoded return values with names if available
                    let mut return_parts = Vec::new();
                    for (i, value) in decoded_values.iter().enumerate() {
                        let param_name = function_abi
                            .outputs
                            .get(i)
                            .map(|param| param.name.as_str())
                            .filter(|name| !name.is_empty());

                        if let Some(name) = param_name {
                            return_parts.push(format!(
                                "{} {}: {}",
                                value.format_type(),
                                name,
                                self.resolve_sol_value(value, false),
                            ));
                        } else {
                            return_parts.push(self.resolve_sol_value(value, true));
                        }
                    }

                    if return_parts.len() == 1 {
                        return Some(return_parts[0].clone());
                    } else {
                        return Some(format!("({})", return_parts.join(", ")));
                    }
                }
                Err(_) => {
                    return None;
                }
            }
        }
        None
    }

    /// Resolve function call
    pub fn resolve_function_call(
        &mut self,
        calldata: &Bytes,
        address: Option<Address>,
    ) -> Option<String> {
        if calldata.len() < 4 {
            return None;
        }

        let selector = Selector::from_slice(&calldata[..4]);
        if let Some(function_abi) = address
            .and_then(|addr| self.get_contract_abi(addr, false))
            .and_then(|abi| abi.function_by_selector(selector).cloned())
        {
            match function_abi.abi_decode_input(&calldata[4..]) {
                Ok(decoded) => {
                    let params: Vec<String> =
                        decoded.iter().map(|param| self.resolve_sol_value(param, true)).collect();

                    return Some(format!("{}({})", function_abi.name, params.join(", ")));
                }
                Err(_) => {
                    return Some(format!(
                        "{}(0x{}...)",
                        function_abi.name,
                        hex::encode(&calldata[4..calldata.len().min(8)])
                    ))
                }
            }
        }

        None
    }

    /// Resolve constructor call
    pub fn resolve_constructor_call(&mut self, address: Address) -> Option<String> {
        let Some(args) = self.get_constructor_args(address) else {
            return None;
        };

        if let Some(abi) =
            self.get_contract_abi(address, false).and_then(|abi| abi.constructor().cloned())
        {
            match abi.abi_decode_input(&args) {
                Ok(decoded) => {
                    let params: Vec<String> =
                        decoded.iter().map(|param| self.resolve_sol_value(param, true)).collect();

                    return Some(format!("constructor({})", params.join(", ")));
                }
                Err(_) => {
                    return None;
                }
            }
        }

        None
    }

    /// Resolve event
    pub fn resolve_event(&mut self, event: &LogData, address: Option<Address>) -> Option<String> {
        if event.topics().is_empty() {
            return None;
        }

        let event_signature = event.topics()[0];
        if let Some(event_abi) = address
            .and_then(|addr| self.get_contract_abi(addr, false))
            .and_then(|abi| abi.events().find(|e| e.selector() == event_signature).cloned())
        {
            // Try to decode the event
            match event_abi.decode_log(event) {
                Ok(decoded) => {
                    // Format decoded event with parameters
                    let mut params = Vec::new();
                    for (param, value) in event_abi.inputs.iter().zip(decoded.body.iter()) {
                        params.push(format!(
                            "{} {}: {}",
                            value.format_type(),
                            param.name,
                            value.format_value(false)
                        ));
                    }

                    if params.is_empty() {
                        return Some(format!("{}()", event_abi.name));
                    } else {
                        return Some(format!("{}({})", event_abi.name, params.join(", ")));
                    }
                }
                Err(_) => {
                    // Fallback to event name with raw data
                    return Some(format!(
                        "{}(0x{}...) [decode failed]",
                        event_abi.name,
                        hex::encode(&event.data[..event.data.len().min(8)])
                    ));
                }
            }
        }

        None
    }

    /// Resolve solidity value
    pub fn resolve_sol_value(&mut self, value: &DynSolValue, with_ty: bool) -> String {
        // TODO: add address label
        value.format_value(with_ty)
    }

    /// Resolve address
    pub fn resolve_address(&mut self, address: Address, readable: bool) -> String {
        // TODO: add address label
        if !readable {
            return format!("{:?}", address);
        } else if address == Address::ZERO {
            "0x0000000000000000".to_string()
        } else {
            let addr_str = format!("{:?}", address);
            // Show more characters for better identification: 8 chars + ... + 6 chars
            format!("{}...{}", &addr_str[..8], &addr_str[addr_str.len() - 6..])
        }
    }

    /// Resolve and format ether
    pub fn resolve_ether(&self, value: U256) -> String {
        // Convert Wei to ETH (1 ETH = 10^18 Wei)
        let eth_value = value.to_string();
        if eth_value.len() <= 18 {
            // Less than 1 ETH - show significant digits only
            let padded = format!("{:0>18}", eth_value);
            let trimmed = padded.trim_end_matches('0');
            if trimmed.is_empty() {
                "0".to_string()
            } else {
                format!("0.{}", &trimmed[..trimmed.len().min(6)])
            }
        } else {
            // More than 1 ETH
            let (whole, decimal) = eth_value.split_at(eth_value.len() - 18);
            let decimal_trimmed = decimal[..4.min(decimal.len())].trim_end_matches('0');
            if decimal_trimmed.is_empty() {
                whole.to_string()
            } else {
                format!("{}.{}", whole, decimal_trimmed)
            }
        }
    }

    /// Synchronize local cache with the shared core
    ///
    /// This is the only async operation in InfoManager, designed to:
    /// - Trigger data fetching in InfoManagerCore
    /// - Update local caches when data is available
    /// - Be called periodically or when fresh data is needed
    pub async fn fetch_data(&mut self) -> Result<()> {
        let mut core = self.core.write().unwrap();
        for request in self.pending_requests.drain() {
            core.fetch_data(request).await?;
        }

        if self.contract_abi.need_update(&core.contract_abi) {
            self.contract_abi.update(&core.contract_abi);
        }

        if self.constructor_args.need_update(&core.constructor_args) {
            self.constructor_args.update(&core.constructor_args);
        }

        Ok(())
    }
}

/// Centralized info state manager handling RPC communication and data fetching
///
/// # Design Philosophy
///
/// `InfoManagerCore` is responsible for:
/// - All RPC communication with the debug server
/// - Complex data fetching and processing
/// - Caching fetched data for distribution to InfoManager instances
/// - Thread-safe state updates via `Arc<RwLock<>>`
///
/// All network I/O and complex operations happen here, keeping
/// InfoManager instances lightweight for rendering.
///
/// # Architecture Benefits
///
/// - **Non-blocking UI**: Rendering threads never wait on RPC calls
/// - **Centralized I/O**: All network operations in one place
/// - **Consistent State**: Single source of truth for fetched data
/// - **Resource Efficiency**: Shared RPC client and cached data
#[derive(Debug, Clone)]
pub struct ResolverCore {
    /// RPC client for server communication
    rpc_client: Arc<RpcClient>,

    /// Cached contract ABI
    contract_abi: FetchCache<(Address, bool), JsonAbi>,

    /// Cached constructor arguments
    constructor_args: FetchCache<Address, Bytes>,
}

impl ResolverCore {
    /// Create a new info manager core with RPC client
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        Self { rpc_client, contract_abi: FetchCache::new(), constructor_args: FetchCache::new() }
    }

    /// Fetch latest data from the debug server
    ///
    /// This method handles all RPC communication and updates
    /// internal caches that InfoManager instances can read
    pub async fn fetch_data(&mut self, request: PendingRequest) -> Result<()> {
        match request {
            PendingRequest::ContractAbi(address, recompiled) => {
                if self.contract_abi.has_cached(&(address, recompiled)) {
                    return Ok(());
                }
                let abi = self.rpc_client.get_contract_abi(address, recompiled).await?;
                self.contract_abi.insert((address, recompiled), abi);
            }
            PendingRequest::ConstructorArgs(address) => {
                if self.constructor_args.has_cached(&address) {
                    return Ok(());
                }
                let args = self.rpc_client.get_constructor_args(address).await?;
                self.constructor_args.insert(address, args);
            }
        }

        Ok(())
    }
}
