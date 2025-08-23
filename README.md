# EDB - Ethereum Debugger 🔍

**Advanced Time-Travel Debugger for Ethereum Transactions**

> ⚠️ **Note**: EDB is currently under active development. Features and APIs may change as we continue to improve the debugging experience.

EDB is a sophisticated step-by-step debugger for Ethereum transactions that provides time-travel debugging capabilities with source-level instrumentation. It allows developers to replay, analyze, and debug Ethereum transactions with unprecedented granularity and control.

## ✨ Key Features

- **🕰️ Time-Travel Debugging**: Navigate through transaction execution history with full state snapshots at every step
- **📝 Source-Level Instrumentation**: Automatically instruments Solidity source code for strategic breakpoint placement
- **🔄 Real Transaction Replay**: Uses REVM for accurate transaction execution with proper hardfork handling
- **⚡ Intelligent Caching**: Multi-level caching system for Etherscan data, RPC responses, and compiled contracts
- **🖥️ Multiple Interfaces**: Both Terminal UI (TUI) and Web UI (coming soon) for different debugging preferences
- **🔧 Dual-Layer Snapshots**: Combines opcode-level and hook-based snapshots for comprehensive debugging coverage
- **🚀 RPC Proxy**: Built-in caching proxy server for improved performance and reliability

## 🚀 Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/MedGa-eth/EDB.git
cd EDB

# Build the project
cargo build --release

# Install the binaries
cargo install --path crates/edb
cargo install --path crates/rpc-proxy
cargo install --path crates/tui
```

### Prerequisites

- **Rust**: 1.88+ (required by latest REVM and Foundry dependencies)
- **Ethereum RPC endpoint**: For blockchain access
- **Etherscan API key**: For fetching verified source code (optional but recommended)

### Basic Usage

Debug a mainnet transaction:

```bash
# Debug with Terminal UI (default)
edb replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060

# Debug with Web UI (experimental)
edb replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060 --ui web

# Quick mode (skip historical replay for recent transactions)
edb replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060 --quick

# Use custom RPC endpoint
edb replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060 --rpc-url https://eth.llamarpc.com
```

### Environment Variables

```bash
# Set RPC endpoint
export ETH_RPC_URL=https://eth.llamarpc.com

# Set Etherscan API key for source download
export ETHERSCAN_API_KEY=your_key_here

# Enable debug logging
export RUST_LOG=debug
```

### RPC Proxy Management

EDB includes an intelligent caching RPC proxy that dramatically improves performance:

```bash
# Check proxy status
edb proxy-status

# The proxy starts automatically when running replay/debug commands
# It runs in the background and caches immutable blockchain data
```

## 🏗️ Architecture Overview

EDB consists of several modular crates working together:

| Crate | Purpose | Status |
|-------|---------|--------|
| **`edb`** | Main CLI orchestrator and workflow management | ✅ Complete |
| **`engine`** | Core debugging engine with instrumentation | 🚧 In Development |
| **`common`** | Shared utilities, types, and forking logic | ✅ Complete |
| **`rpc-proxy`** | Caching proxy for RPC performance | ✅ Complete |
| **`tui`** | Terminal-based debugging interface | 🚧 In Development |
| **`webui`** | Browser-based debugging interface | 📋 Planned |

For detailed architecture documentation, see [ARCH.md](./ARCH.md).

## 🔬 How It Works

### Debugging Workflow

1. **Transaction Replay**: EDB replays the target transaction using REVM with the exact blockchain state at that block
2. **Trace Collection**: Captures all contract interactions and call traces during execution
3. **Source Download**: Fetches verified source code from Etherscan for all touched contracts
4. **Code Analysis**: Analyzes Solidity AST to identify execution steps, variable scopes, and instrumentation points
5. **Instrumentation**: Inserts strategic debugging hooks into the source code
6. **Recompilation**: Compiles instrumented contracts with debugging symbols
7. **Bytecode Tweaking**: Replaces original bytecode with instrumented versions for debugging
8. **Snapshot Collection**: Captures comprehensive state snapshots at both opcode and source levels
9. **Interactive Debugging**: Provides time-travel debugging through TUI or Web interface with RPC API

### Key Innovations

- **Dual-Layer Snapshot System**: Intelligently merges opcode-level and hook-based snapshots
- **Smart Bytecode Tweaking**: Downloads contract creation transactions and replaces deployed bytecode
- **Real EVM Execution**: Uses REVM's `transact_commit()` for accurate transaction replay
- **Comprehensive Caching**: Multi-level caching at Etherscan, RPC, and compilation layers

## 🖥️ User Interfaces

### Terminal UI (TUI)

The TUI provides a rich terminal-based debugging experience:

- **Code Panel**: Syntax-highlighted Solidity source with current execution position
- **Trace Panel**: Interactive call hierarchy and execution trace
- **Terminal Panel**: Command input and debugging output
- **Display Panel**: Variable inspection and state visualization

**Controls:**
- `n` / `Space` - Step to next execution point
- `N` - Step backward
- `c` - Continue to next breakpoint
- `↑`/`↓` - Navigate trace
- `Tab` - Switch panels
- `q` - Quit debugger

### Web UI (Coming Soon)

The Web UI will provide:
- Visual execution flow diagram
- Interactive state inspector
- Source code with inline variable values
- Advanced breakpoint management
- Watch expressions

## 🛠️ Development

See [DEV.md](./DEV.md) for detailed development instructions, including:
- Building from source
- Running tests
- Contributing guidelines
- Architecture details
- Adding new features

### Quick Development Setup

```bash
# Run all tests
cargo test --workspace

# Run with debug logging
RUST_LOG=debug cargo run -- replay 0x...

# Check code quality
cargo clippy --all-targets --all-features
cargo fmt --check
```

## 📊 Performance

EDB is optimized for performance with:
- **RPC Proxy Caching**: Reduces network calls by caching immutable data
- **Parallel Processing**: Uses async/await for concurrent operations
- **Smart Caching**: TTL-based caching for different data types
- **Efficient State Management**: Optimized snapshot storage and retrieval

## 🧪 Testing

EDB includes comprehensive test coverage:

```bash
# Unit tests
cargo test --lib

# Integration tests (requires RPC)
ETH_RPC_URL=https://eth.llamarpc.com cargo test --test '*' -- --ignored

# Specific test suites
cargo test -p edb-common --test forking_tests
cargo test -p edb-engine --test source_tests
```

## 📄 License

EDB is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0).

Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

See [LICENSE](./LICENSE) for the full license text.

For commercial licensing inquiries without AGPL restrictions, please contact the authors.

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

Areas where we especially welcome contributions:
- Web UI implementation
- Additional blockchain support
- Performance optimizations
- Documentation improvements
- Bug fixes and testing

## 🙏 Acknowledgments

- Built with [Alloy](https://github.com/alloy-rs/alloy) and [REVM](https://github.com/bluealloy/revm)
- Inspired by traditional debuggers and the Ethereum development community
- Special thanks to the Foundry and Reth teams for their excellent libraries

## 📬 Contact

For questions, feedback, or support:
- Open an issue on [GitHub](https://github.com/MedGa-eth/EDB/issues)
- Contact the authors: Zhuo Zhang and Wuqi Zhang

---

*This documentation was crafted with Claude with Love ❤️*