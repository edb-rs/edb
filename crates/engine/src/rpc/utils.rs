//! RPC server utilities
//!
//! This module provides utility functions for RPC server operations,
//! including port discovery and helper functions.

use eyre::{eyre, Result};
use std::net::{SocketAddr, TcpListener};
use tracing::{debug, info};

/// Find an available port starting from a base port
pub fn find_available_port(start_port: u16) -> Result<u16> {
    for port in start_port..65535 {
        if is_port_available(port) {
            info!("Found available port: {}", port);
            return Ok(port);
        }
    }
    Err(eyre!("No available port found in range {}-65534", start_port))
}

/// Check if a port is available on localhost
pub fn is_port_available(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => {
            debug!("Port {} is available", port);
            true
        }
        Err(_) => {
            debug!("Port {} is not available", port);
            false
        }
    }
}

/// Get default RPC server port (tries 3000 first, then searches)
pub fn get_default_rpc_port() -> Result<u16> {
    if is_port_available(3000) {
        Ok(3000)
    } else {
        find_available_port(3001)
    }
}

/// Parse a socket address, with sensible defaults
pub fn parse_socket_addr(addr_str: Option<&str>, default_port: u16) -> Result<SocketAddr> {
    match addr_str {
        Some(addr) => addr.parse().map_err(|e| eyre!("Invalid socket address '{}': {}", addr, e)),
        None => Ok(SocketAddr::from(([127, 0, 0, 1], default_port))),
    }
}

/// Convert error to RPC error format
pub fn to_rpc_error(
    code: i32,
    message: &str,
    data: Option<serde_json::Value>,
) -> crate::rpc::types::RpcError {
    crate::rpc::types::RpcError { code, message: message.to_string(), data }
}

/// Helper to create internal error responses
pub fn internal_error(message: &str) -> crate::rpc::types::RpcError {
    to_rpc_error(crate::rpc::types::error_codes::INTERNAL_ERROR, message, None)
}

/// Helper to create method not found error
pub fn method_not_found(method: &str) -> crate::rpc::types::RpcError {
    to_rpc_error(
        crate::rpc::types::error_codes::METHOD_NOT_FOUND,
        &format!("Method '{}' not found", method),
        None,
    )
}

/// Helper to create invalid params error
pub fn invalid_params(message: &str) -> crate::rpc::types::RpcError {
    to_rpc_error(crate::rpc::types::error_codes::INVALID_PARAMS, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_availability() {
        // Test with a port that should be available
        let port = find_available_port(50000).expect("Should find an available port");
        assert!(port >= 50000);
        assert!(is_port_available(port));
    }

    #[test]
    fn test_socket_addr_parsing() {
        let addr = parse_socket_addr(Some("127.0.0.1:8080"), 3000).unwrap();
        assert_eq!(addr.port(), 8080);

        let addr = parse_socket_addr(None, 3000).unwrap();
        assert_eq!(addr.port(), 3000);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn test_error_helpers() {
        let err = internal_error("test message");
        assert_eq!(err.code, -32603);
        assert_eq!(err.message, "test message");

        let err = method_not_found("test_method");
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("test_method"));
    }
}
