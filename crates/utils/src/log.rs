//! Logging utilities for testing and debugging.
#![allow(clippy::test_attr_in_doctest)]

use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();

/// Initialize tracing subscriber for console logging.
///
/// This function sets up a tracing subscriber that prints logs to the console
/// with a default level of INFO. The log level can be controlled via the
/// `RUST_LOG` environment variable.
///
/// This function is safe to call multiple times - it will only initialize
/// the subscriber once using `Once::call_once`.
///
/// # Examples
///
/// ```rust
/// use edb_utils::init_test_tracing;
///
/// #[test]
/// fn my_test() {
///     init_test_tracing();
///     tracing::info!("This will be printed to console");
///     // ... test code
/// }
/// ```
///
/// # Environment Variables
///
/// - `RUST_LOG`: Controls the log level (e.g., "debug", "info", "warn", "error")
///   - Example: `RUST_LOG=debug cargo test`
///   - Example: `RUST_LOG=edb_utils=trace cargo test`
pub fn init_test_tracing() {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        fmt()
            .with_env_filter(filter)
            .with_test_writer()
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .compact()
            .init();
    });
}

/// Initialize tracing subscriber with a specific log level.
///
/// This is useful when you want to force a specific log level regardless
/// of the `RUST_LOG` environment variable.
///
/// # Examples
///
/// ```rust
/// use edb_utils::init_test_tracing_with_level;
///
/// #[test]
/// fn my_debug_test() {
///     init_test_tracing_with_level("debug");
///     tracing::debug!("This debug message will be shown");
///     // ... test code
/// }
/// ```
pub fn init_test_tracing_with_level(level: &str) {
    INIT.call_once(|| {
        let is_debug = level.to_lowercase().contains("debug");
        let filter = EnvFilter::new(level);

        fmt()
            .with_env_filter(filter)
            .with_test_writer()
            .with_target(is_debug)
            .with_thread_ids(is_debug)
            .with_file(is_debug)
            .with_line_number(is_debug)
            .compact()
            .init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_test_tracing() {
        init_test_tracing();

        tracing::info!("This will be printed to console");
    }
}
