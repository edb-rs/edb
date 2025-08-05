# EDB Development Guide

This document provides comprehensive guidance for developing the Ethereum Debugger (EDB).

## Prerequisites

### System Requirements
- **Rust**: 1.88+ (upgraded from 1.87 for latest Foundry compatibility)
- **Git**: For version control
- **Ethereum RPC Access**: For testing with real transactions

### Dependencies
EDB uses Foundry's exact dependency versions to ensure compatibility:
- **alloy-primitives**: 1.3.0
- **alloy-provider**: 1.0.23
- **alloy-rpc-types**: 1.0.23
- **revm**: 27.1.0
- **foundry-compilers**: Latest from Foundry's Cargo.toml

## Project Structure

```
EDB/
├── Cargo.toml              # Workspace configuration
├── crates/
│   ├── edb/                # Main binary crate
│   │   ├── src/main.rs     # CLI and orchestration
│   │   └── Cargo.toml
│   ├── engine/             # Analysis and instrumentation
│   │   ├── src/
│   │   │   ├── lib.rs      # Core analyze() function
│   │   │   ├── source.rs   # Source code download
│   │   │   ├── compiler.rs # Contract recompilation
│   │   │   ├── analysis/   # Analysis modules
│   │   │   ├── instrumentation/ # Standalone directory
│   │   │   └── rpc/        # JSON-RPC server (standalone)
│   │   └── Cargo.toml
│   ├── utils/              # Chain interaction utilities
│   │   ├── src/
│   │   │   ├── lib.rs      # Types and public interface
│   │   │   └── forking.rs  # Chain forking implementation
│   │   └── Cargo.toml
│   ├── tui/                # Terminal UI (skeleton)
│   │   └── src/lib.rs
│   └── webui/              # Web UI (skeleton)
│       └── src/lib.rs
├── arch.md                 # Architecture documentation
├── dev.md                  # This file
└── README.md               # User-facing documentation
```

## Development Workflow

### Before Pushing to GitHub

Please ensure the following commands pass if you have changed the code:

```bash
# Check compilation
cargo check --all

# Run tests (when implemented)
cargo test --all --all-features

# Format code
cargo +nightly fmt -- --check

# Lint code
cargo clippy --all --all-targets --all-features -- -D warnings

# Build release version
cargo build --release
```

### Testing

#### Running the Debugger
```bash
# Basic usage - replay a transaction
cargo run -- replay 0x<transaction_hash>

# With debug logging
RUST_LOG=debug cargo run -- replay 0x<transaction_hash>

# Use web UI instead of TUI
cargo run -- --ui web replay 0x<transaction_hash>

# Specify custom RPC endpoint
cargo run -- --rpc-url https://mainnet.infura.io/v3/YOUR_KEY replay 0x<transaction_hash>
```

#### Testing Chain Forking
```bash
# Test with a known transaction (requires RPC access)
RUST_LOG=debug cargo run -- replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060
```

## Development Areas

### 1. Utils Crate (`crates/utils`)

**Status: ✅ Complete**

The utils crate handles all chain interaction:
- `fork_and_prepare()`: Creates chain forks using Alloy provider
- `replay_transaction()`: Analyzes transaction receipts
- `analyze_block_transactions()`: Finds transaction dependencies

**Key Implementation Details:**
- Uses `ProviderBuilder::new().connect(&rpc_url).await?` for RPC connections
- Accesses transaction fields via `tx.inner.hash()` API
- Handles receipts with `receipt.logs()` method calls
- Returns `ForkResult` with placeholder database/environment types

### 2. Engine Crate (`crates/engine`)

**Status: 🚧 Partial Implementation**

The engine accepts pre-forked inputs and performs analysis:
- `analyze()` function signature is complete
- Placeholder implementations for most modules
- RPC server structure is in place

**Areas for Development:**
- **Source Download** (`source.rs`): Implement Etherscan API integration
- **Instrumentation** (`instrumentation/`): Solidity parsing and precompile injection
- **Compiler** (`compiler.rs`): Contract recompilation with foundry-compilers
- **Analysis** (`analysis/`): Complete visitor pattern implementation

### 3. EDB Binary (`crates/edb`)

**Status: ✅ Complete**

The main binary orchestrates the workflow:
- CLI parsing with clap
- Calls utils for forking
- Calls engine for analysis
- Launches UI

### 4. UI Crates

**Status: 📋 Skeleton Only**

Both TUI and WebUI crates have basic structure but need implementation.

## Key Development Patterns

### Error Handling
```rust
use eyre::Result;

pub async fn my_function() -> Result<ReturnType> {
    // Use ? operator for error propagation
    let result = some_operation().await?;
    Ok(result)
}
```

### Logging
```rust
use tracing::{info, debug, warn, error};

pub fn my_function() {
    info!("High-level operation started");
    debug!("Detailed debugging info");
    warn!("Something concerning happened");
    error!("Something went wrong: {}", error_message);
}
```

### Async Provider Usage
```rust
use alloy_provider::ProviderBuilder;

let provider = ProviderBuilder::new()
    .connect(&rpc_url)
    .await?;

let tx = provider
    .get_transaction_by_hash(tx_hash)
    .await?
    .ok_or_else(|| eyre::eyre!("Transaction not found"))?;
```

## Common Issues and Solutions

### 1. Alloy API Changes
**Problem**: Transaction fields moved to `tx.inner.*`
**Solution**: Access via `tx.inner.hash()`, `tx.inner.from()`, etc.

### 2. REVM Compatibility
**Problem**: REVM types changed between versions
**Solution**: Use placeholder types in utils crate, convert at engine boundary

### 3. Provider Methods
**Problem**: `get_block_by_number` signature changed
**Solution**: Only pass block number, not transaction kind

### 4. Async Context
**Problem**: All provider methods are async
**Solution**: Ensure functions are `async` and use `.await?`

## Git Commit Guidelines

Use conventional commit messages:

+ **feat**: A new feature for the user
+ **fix**: A bug fix
+ **docs**: Documentation only changes
+ **style**: Code style changes (formatting, etc.)
+ **refactor**: Code restructuring without behavior change
+ **perf**: Performance improvements
+ **test**: Adding or updating tests
+ **chore**: Build system or tooling changes
+ **ci**: CI configuration changes
+ **build**: Build system or dependency changes
+ **revert**: Reverting a previous commit

Examples:
- `feat: implement transaction replay in utils crate`
- `fix: resolve alloy provider API compatibility issues`
- `docs: update architecture documentation with utils crate`
- `refactor: separate forking logic into utils crate`
