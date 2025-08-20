//! JSON-RPC server for debugging control
//!
//! This module provides a JSON-RPC interface for front-ends to control
//! and inspect debugging sessions.

pub mod methods;
pub mod server;
pub mod types;
pub mod utils;

pub use server::*;
pub use types::*;
