# EDB Development Guide

This comprehensive guide provides everything you need to develop, test, and contribute to the Ethereum Debugger (EDB) project.

## 🛠️ Prerequisites

### System Requirements

- **Rust**: 1.88+ (required for REVM v27 and latest Foundry dependencies)
- **Git**: For version control
- **Cargo**: Rust's package manager (comes with Rust)
- **Make**: Optional, for convenience commands (future)

### External Dependencies

- **Ethereum RPC Endpoint**: Required for blockchain interaction
  - Public: `https://eth.llamarpc.com` (for testing)
  - Private: Infura, Alchemy, or local node
- **Etherscan API Key**: For downloading verified source code
  - Get free key at https://etherscan.io/apis

## 📁 Project Structure

```
EDB/
├── Cargo.toml                 # Workspace configuration
├── crates/
│   ├── edb/                   # Main CLI binary
│   │   ├── src/
│   │   │   ├── main.rs        # Entry point and CLI parsing
│   │   │   ├── proxy.rs       # RPC proxy management
│   │   │   ├── utils.rs       # Binary discovery utilities
│   │   │   └── cmd/           # Command implementations
│   │   │       ├── mod.rs
│   │   │       ├── replay.rs  # Transaction replay command
│   │   │       ├── debug.rs   # Foundry test debugging
│   │   │       └── proxy_status.rs
│   │   └── Cargo.toml
│   │
│   ├── common/                # Shared utilities
│   │   ├── src/
│   │   │   ├── lib.rs         # Public API
│   │   │   ├── cache.rs       # Caching infrastructure
│   │   │   ├── context.rs     # EVM context wrappers
│   │   │   ├── forking.rs     # Chain forking with REVM
│   │   │   ├── logging.rs     # Fancy logging setup
│   │   │   ├── opcode.rs      # Opcode analysis utilities
│   │   │   ├── spec_id.rs     # Hardfork mapping
│   │   │   └── types/         # Common types
│   │   │       ├── mod.rs
│   │   │       ├── trace.rs   # Trace structures
│   │   │       └── execution_frame.rs
│   │   ├── tests/
│   │   │   └── forking_tests.rs
│   │   └── Cargo.toml
│   │
│   ├── engine/                # Core debugging engine
│   │   ├── src/
│   │   │   ├── lib.rs         # Public API
│   │   │   ├── core.rs        # Main engine orchestration
│   │   │   ├── context.rs     # Engine context and state
│   │   │   ├── snapshot.rs    # Dual-layer snapshots
│   │   │   ├── source.rs      # Source code download
│   │   │   ├── tweak.rs       # Bytecode replacement
│   │   │   ├── analysis/      # AST analysis
│   │   │   │   ├── mod.rs
│   │   │   │   ├── analyzer.rs # Main analyzer
│   │   │   │   ├── annotation.rs
│   │   │   │   ├── common.rs
│   │   │   │   ├── hook.rs    # Hook placement
│   │   │   │   ├── step.rs    # Execution steps
│   │   │   │   ├── variable.rs # Variable tracking
│   │   │   │   └── visitor.rs # AST visitor
│   │   │   ├── inspector/     # REVM inspectors
│   │   │   │   ├── mod.rs
│   │   │   │   ├── call_tracer.rs
│   │   │   │   ├── hook_snapshot_inspector.rs
│   │   │   │   ├── opcode_snapshot_inspector.rs
│   │   │   │   └── tweak_inspector.rs
│   │   │   ├── instrumentation/ # Code instrumentation
│   │   │   │   ├── mod.rs
│   │   │   │   └── common.rs
│   │   │   ├── rpc/           # JSON-RPC server
│   │   │   │   ├── mod.rs
│   │   │   │   ├── server.rs
│   │   │   │   ├── types.rs
│   │   │   │   ├── utils.rs
│   │   │   │   └── methods/
│   │   │   │       ├── mod.rs
│   │   │   │       ├── navigation.rs
│   │   │   │       └── trace.rs
│   │   │   └── utils/         # Engine utilities
│   │   │       ├── mod.rs
│   │   │       ├── artifact.rs
│   │   │       ├── ast_prune.rs
│   │   │       ├── disasm.rs
│   │   │       ├── etherscan.rs
│   │   │       └── onchain_compiler.rs
│   │   ├── tests/
│   │   │   ├── config_tests.rs
│   │   │   └── source_tests.rs
│   │   └── Cargo.toml
│   │
│   ├── rpc-proxy/             # Caching RPC proxy
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cache.rs       # Cache implementation
│   │   │   ├── health.rs      # Health monitoring
│   │   │   ├── metrics.rs     # Performance metrics
│   │   │   ├── providers.rs   # Provider management
│   │   │   ├── proxy.rs       # Proxy server
│   │   │   ├── registry.rs    # Provider registry
│   │   │   ├── rpc.rs         # RPC handling
│   │   │   ├── bin/
│   │   │   │   └── main.rs    # Standalone proxy binary
│   │   │   └── tui/           # Proxy monitoring UI
│   │   │       ├── mod.rs
│   │   │       ├── app.rs
│   │   │       ├── remote.rs
│   │   │       └── widgets.rs
│   │   ├── tests/
│   │   │   └── integration_tests.rs
│   │   └── Cargo.toml
│   │
│   ├── tui/                   # Terminal UI
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── app.rs         # Main TUI application
│   │   │   ├── config.rs      # Configuration
│   │   │   ├── layout.rs      # Panel layout
│   │   │   ├── rpc.rs         # RPC client
│   │   │   ├── bin/
│   │   │   │   └── main.rs    # TUI binary
│   │   │   ├── managers/      # Resource management
│   │   │   │   ├── mod.rs
│   │   │   │   ├── execution.rs
│   │   │   │   ├── resource.rs
│   │   │   │   └── theme.rs
│   │   │   ├── panels/        # UI panels
│   │   │   │   ├── mod.rs
│   │   │   │   ├── code.rs    # Source code display
│   │   │   │   ├── display.rs # Variable display
│   │   │   │   ├── terminal.rs # Command input
│   │   │   │   └── trace.rs   # Execution trace
│   │   │   └── ui/            # UI components
│   │   │       ├── mod.rs
│   │   │       ├── borders.rs
│   │   │       ├── colors.rs
│   │   │       ├── icons.rs
│   │   │       ├── spinner.rs
│   │   │       ├── status.rs
│   │   │       └── syntax/    # Syntax highlighting
│   │   │           ├── mod.rs
│   │   │           ├── opcodes.rs
│   │   │           └── solidity.rs
│   │   └── Cargo.toml
│   │
│   ├── webui/                 # Web UI (planned)
│   │   ├── src/
│   │   │   └── lib.rs         # Axum server skeleton
│   │   └── Cargo.toml
│   │
│   └── integration-tests/     # End-to-end tests
│       ├── tests/
│       │   └── forking_with_proxy_tests.rs
│       └── Cargo.toml
│
├── testdata/                  # Test data and cache
├── LICENSE                    # AGPL-3.0 license
├── COPYRIGHT                  # Copyright notice
├── README.md                  # User documentation
├── ARCH.md                    # Architecture documentation
├── DEV.md                     # This file
└── CONTRIBUTING.md            # Contribution guidelines
```

## 📝 TODO

- [ ] Conditional breakpoints
- [ ] Complex type local variable watcher (user-defined type, etc.)
- [ ] Customized watcher

## 🚀 Getting Started

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/MedGa-eth/EDB.git
cd EDB

# Build all crates
cargo build --workspace

# Run tests to verify setup
cargo test --workspace

# Install binaries locally
cargo install --path crates/edb
cargo install --path crates/rpc-proxy
cargo install --path crates/tui
```

### Environment Configuration

Create a `.env` file in the project root:

```env
# Ethereum RPC endpoint (required)
ETH_RPC_URL=https://eth.llamarpc.com

# Etherscan API key (recommended)
ETHERSCAN_API_KEY=your_key_here

# Logging configuration
RUST_LOG=info,edb=debug,edb_engine=debug

# Cache directory (optional)
EDB_CACHE_DIR=/tmp/edb_cache
```

## 🧪 Testing

### Running Tests

```bash
# Run all unit tests
cargo test --workspace --lib

# Run all tests including integration tests
cargo test --workspace --all

# Run specific crate tests
cargo test -p edb-common
cargo test -p edb-engine
cargo test -p edb-rpc-proxy

# Run with debug output
RUST_LOG=debug cargo test -- --nocapture

# Run ignored tests (requires RPC)
ETH_RPC_URL=https://eth.llamarpc.com cargo test -- --ignored
```

### Test Coverage Areas

#### Common Crate Tests
- **Forking Tests**: Real transaction replay with REVM
- **SpecId Tests**: Hardfork mapping verification
- **Cache Tests**: TTL and persistence testing
- **Context Tests**: EVM wrapper functionality

#### Engine Crate Tests
- **Analysis Tests**: AST parsing and step identification
- **Source Tests**: Etherscan download and caching
- **Instrumentation Tests**: Hook insertion verification
- **Snapshot Tests**: State capture accuracy

#### RPC Proxy Tests
- **Cache Tests**: Response caching behavior
- **Provider Tests**: Load balancing and failover
- **Health Tests**: Provider health monitoring

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Unit test example
        let result = my_function();
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_functionality() {
        // Async test example
        let result = async_function().await.unwrap();
        assert!(result.is_valid());
    }

    #[test]
    #[ignore] // Run with --ignored flag
    fn test_requiring_rpc() {
        // Integration test requiring RPC
        let rpc_url = std::env::var("ETH_RPC_URL")
            .expect("ETH_RPC_URL required");
        // Test implementation
    }
}
```

## 💻 Development Workflow

### Before Committing

Run the pre-commit checklist:

```bash
# Format code
cargo fmt --all

# Check linting
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --workspace

# Check compilation for all targets
cargo check --all-targets --all-features

# Build documentation
cargo doc --no-deps --workspace
```

### Debugging the Debugger

```bash
# Run with maximum verbosity
RUST_LOG=trace cargo run -- replay 0x...

# Run with specific module debugging
RUST_LOG=edb_engine::core=trace cargo run -- replay 0x...

# Use debugger (with lldb)
rust-lldb target/debug/edb -- replay 0x...

# Profile performance
cargo build --release
perf record --call-graph=dwarf target/release/edb replay 0x...
perf report
```

## 🏗️ Architecture Guidelines

### Module Responsibilities

#### Common Crate
- **Purpose**: Shared utilities with no domain logic
- **Dependencies**: Minimal, only essential libraries
- **Exports**: Types, traits, and utility functions

#### Engine Crate
- **Purpose**: Core debugging logic and analysis
- **Dependencies**: Common crate, Foundry libraries
- **State**: Immutable after preparation

#### UI Crates (TUI/WebUI)
- **Purpose**: User interaction and visualization
- **Dependencies**: Engine RPC client
- **State**: Manages UI state, not debugging state

### Code Patterns

#### Error Handling
```rust
use eyre::{Result, eyre, Context};

pub fn risky_operation() -> Result<Value> {
    let data = fetch_data()
        .context("Failed to fetch data")?;

    let processed = process_data(data)
        .map_err(|e| eyre!("Processing failed: {}", e))?;

    Ok(processed)
}
```

#### Async Patterns
```rust
use tokio::time::{sleep, Duration};

pub async fn retry_operation<F, T>(
    mut f: F,
    max_retries: usize,
) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    for attempt in 0..max_retries {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_retries - 1 => {
                warn!("Attempt {} failed: {}", attempt + 1, e);
                sleep(Duration::from_secs(1 << attempt)).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

#### REVM Integration
```rust
use revm::{Context, MainnetEvm};
use edb_common::EdbContext;

pub fn execute_with_inspector<I>(
    ctx: EdbContext,
    inspector: &mut I,
) -> Result<ExecutionResult>
where
    I: Inspector<CTX = Context>,
{
    let mut evm = ctx.build_mainnet_with_inspector(inspector);
    let result = evm.transact_commit(tx_env)?;
    Ok(result)
}
```

## 🔧 Common Tasks

### Adding a New RPC Method

1. Define the method in `engine/src/rpc/types.rs`:
```rust
#[derive(Serialize, Deserialize)]
pub struct MyMethodParams {
    pub param1: String,
    pub param2: u64,
}
```

2. Implement handler in `engine/src/rpc/methods/`:
```rust
pub async fn handle_my_method(
    params: MyMethodParams,
    context: &EngineContext,
) -> Result<JsonValue> {
    // Implementation
}
```

3. Register in method dispatcher:
```rust
match method {
    "edb_myMethod" => handle_my_method(params, &self.context).await,
    // ...
}
```

### Adding a New Inspector

1. Create inspector in `engine/src/inspector/`:
```rust
pub struct MyInspector {
    // State fields
}

impl Inspector for MyInspector {
    type CTX = Context;

    fn step(&mut self, interp: &mut Interpreter, ctx: &mut Self::CTX) {
        // Capture state
    }
}
```

2. Integrate into engine workflow:
```rust
let mut inspector = MyInspector::new();
let result = ctx.build_mainnet_with_inspector(&mut inspector)
    .transact_commit(tx_env)?;
```

### Adding UI Components

#### TUI Panel
```rust
// In tui/src/panels/
pub struct MyPanel {
    // Panel state
}

impl Panel for MyPanel {
    fn render(&mut self, f: &mut Frame, area: Rect) {
        // Render logic
    }

    fn handle_input(&mut self, key: KeyEvent) -> Result<()> {
        // Input handling
    }
}
```

## 🐛 Troubleshooting

### Common Issues

#### "No transport enabled" Error
**Solution**: Enable `reqwest` feature in Cargo.toml:
```toml
alloy-provider = { version = "...", features = ["reqwest"] }
```

#### REVM API Changes
**Solution**: Use new Context-based API:
```rust
// Old: evm.into_context()
// New: evm.ctx
```

#### Transaction Field Access
**Solution**: Use accessor methods:
```rust
// Old: tx.gas_limit
// New: tx.gas_limit()
```

#### Compilation Errors with Dependencies
**Solution**: Ensure all Foundry dependencies use same versions:
```bash
cargo tree -d  # Check for duplicate dependencies
```

### Performance Issues

#### Slow Transaction Replay
- Use `--quick` mode for recent transactions
- Ensure RPC proxy is running for caching
- Consider using local node for frequent debugging

#### High Memory Usage
- Limit snapshot collection with selective instrumentation
- Use streaming for large source files
- Clear cache periodically

## 📝 Code Style

### Rust Guidelines

- Use `rustfmt` for consistent formatting
- Follow Rust API guidelines: https://rust-lang.github.io/api-guidelines/
- Prefer explicit types for public APIs
- Document all public items with doc comments

### Git Commit Convention

Use conventional commits:

```
type(scope): description

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `perf`: Performance improvement
- `test`: Testing
- `chore`: Maintenance
- `build`: Build system

Examples:
```bash
git commit -m "feat(engine): add dual-layer snapshot system"
git commit -m "fix(tui): correct panel layout calculation"
git commit -m "docs: update development guide with REVM v27"
```

## 🚢 Release Process

### Version Bumping

```bash
# Update version in all Cargo.toml files
cargo set-version 0.2.0

# Update CHANGELOG.md
# Create git tag
git tag -a v0.2.0 -m "Release version 0.2.0"
```

### Publishing (Future)

```bash
# Dry run
cargo publish --dry-run -p edb-common

# Publish in dependency order
cargo publish -p edb-common
cargo publish -p edb-engine
cargo publish -p edb-rpc-proxy
cargo publish -p edb-tui
cargo publish -p edb
```

## 📚 Resources

### Documentation
- [REVM Documentation](https://github.com/bluealloy/revm)
- [Alloy Documentation](https://github.com/alloy-rs/alloy)
- [Foundry Book](https://book.getfoundry.sh/)
- [Ethereum Yellow Paper](https://ethereum.github.io/yellowpaper/paper.pdf)

### Tools
- [Etherscan API](https://docs.etherscan.io/)
- [Cast (Foundry CLI)](https://book.getfoundry.sh/cast/)
- [Tenderly Debugger](https://tenderly.co/) (for comparison)

### Community
- [EDB GitHub Issues](https://github.com/MedGa-eth/EDB/issues)
- [Foundry Discord](https://discord.gg/foundry)
- [Ethereum StackExchange](https://ethereum.stackexchange.com/)

---

*This development guide was crafted with Claude with Love ❤️*
