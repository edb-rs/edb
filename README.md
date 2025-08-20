# EDB - Ethereum Debugger

A powerful step-by-step debugger for Ethereum transactions, providing deep visibility into smart contract execution.

_This document is created by Claude with ❤️._ 

**The current open-source version is under construction.**

## Features

- 🔍 **Transaction Replay**: Debug any historical transaction by its hash
- 🧪 **Test Debugging**: Debug Foundry test cases directly
- 📝 **Source-Level Debugging**: Step through Solidity code, not bytecode
- 🎯 **State Inspection**: View contract state, stack, memory at each step
- 🖥️ **Multiple UIs**: Choose between Terminal UI or Web UI
- 🔧 **Instrumentation**: Automatic contract instrumentation for debugging

## Installation

### From Source

```bash
git clone https://github.com/MedGa-eth/EDB.git
cd EDB
cargo install --path crates/edb
```

### Prerequisites

- Rust 1.88+ (required by latest foundry dependencies)
- An Ethereum RPC endpoint
- Etherscan API key (for verified contract source)

## Quick Start

### Debug a Transaction

```bash
# Debug a mainnet transaction
edb replay 0x1234567890abcdef... --rpc-url https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY

# With Etherscan API key for source code
export ETHERSCAN_API_KEY=your_key_here
edb replay 0x1234567890abcdef...
```

### Debug a Foundry Test

```bash
# Debug a specific test case
edb test testTransferFunction --rpc-url http://localhost:8545
```

### UI Options

```bash
# Use Terminal UI (default)
edb replay 0x... --ui tui

# Use Web UI
edb replay 0x... --ui web
```

## Configuration

### Environment Variables

Create a `.env` file in your project root:

```env
# Ethereum RPC endpoint
ETH_RPC_URL=http://localhost:8545

# Etherscan API key for source download
ETHERSCAN_API_KEY=your_api_key_here

# Logging level
RUST_LOG=info
```

### Command Line Options

```
edb [OPTIONS] <COMMAND>

Commands:
  replay  Replay an existing transaction
  test    Debug a Foundry test case

Options:
  --rpc-url <URL>      Ethereum RPC endpoint [env: ETH_RPC_URL]
  --ui <MODE>          User interface (tui/web) [default: tui]
  --block <NUMBER>     Block number to fork at
  --port <PORT>        Port for JSON-RPC server [default: 8545]
  --help               Print help
```

## How It Works

1. **Fork Creation**: EDB creates a local fork of the blockchain at the transaction's block
2. **Contract Analysis**: Identifies all contracts involved in the transaction
3. **Source Download**: Fetches verified source code from Etherscan
4. **Instrumentation**: Injects debugging hooks into the source code
5. **Recompilation**: Compiles instrumented contracts
6. **Execution**: Replays transaction with debugging enabled
7. **UI Connection**: Provides step-by-step control through chosen UI

## Debugging Interface

### Terminal UI Controls

- `n` - Step to next execution point
- `c` - Continue to next breakpoint
- `q` - Quit debugger
- `↑/↓` - Navigate stack frames
- `Tab` - Switch between panels

### Web UI Features

- Visual execution flow
- Interactive state inspector
- Source code highlighting
- Breakpoint management
- Variable watches

## Architecture

EDB consists of five main components:

- **EDB Binary**: CLI interface and workflow orchestration
- **Utils Crate**: Chain forking and transaction replay
- **Engine Crate**: Core analysis and instrumentation logic
- **TUI Crate**: Terminal-based debugging interface (skeleton)
- **WebUI Crate**: Browser-based debugging interface (skeleton)

See [arch.md](arch.md) for detailed architecture documentation.

## Implementation Status

### ✅ Completed Features
- **CLI Interface**: Full argument parsing and command handling
- **Chain Forking**: Complete implementation with Alloy provider integration
- **Transaction Analysis**: Receipt analysis and contract discovery
- **Project Structure**: 5-crate workspace with proper separation

### 🚧 In Development
- **Engine Analysis**: Core debugging logic (partial implementation)
- **Source Download**: Etherscan API integration framework
- **Contract Instrumentation**: Solidity parsing and precompile injection
- **RPC Server**: JSON-RPC interface for UI communication

### 📋 Planned Features
- **UI Implementation**: Complete TUI and WebUI interfaces
- **Real Transaction Testing**: Integration with live Ethereum data
- **Advanced Debugging**: Breakpoints, watch expressions, time-travel

## Development

See [dev.md](dev.md) for development setup and guidelines.

### Building from Source

```bash
# Build all components
cargo build --workspace

# Run tests (when implemented)
cargo test --workspace

# Test chain forking functionality (requires RPC access)
RUST_LOG=debug cargo run --bin edb -- replay 0x5c504ed432cb51138bcf09aa5e8a410dd4a1e204ef84bfed1be16dfba1b22060

# Build optimized release version
cargo build --release
```

### Current Capabilities

As of the current implementation, EDB can:
- Parse command-line arguments and identify target transactions
- Connect to Ethereum RPC endpoints using Alloy
- Fork chains at specific blocks and analyze transaction dependencies  
- Extract touched contract addresses from transaction receipts
- Set up the foundation for source download and instrumentation

**Note**: Complete end-to-end debugging is not yet functional as the engine analysis needs completion.

## Current Limitations

### Implementation Status
- **UI interfaces are skeleton implementations only** - debugging interface not yet functional
- **Engine analysis is partially implemented** - source download and instrumentation need completion
- **Testing limited to compilation** - real transaction debugging not yet tested

### Technical Limitations  
- Only supports transactions with verified source code
- Instrumentation may increase gas usage
- Some complex contracts may fail to recompile
- Limited to single transaction debugging (no cross-transaction state)
- Requires Ethereum RPC access for chain forking

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md).

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

Copyright (C) 2024 Zhuo Zhang and Wuqi Zhang

EDB is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0).

This means:
- ✅ You can use, modify, and distribute this software
- ✅ You can use it for commercial purposes
- ⚠️ You must disclose your source code when distributing
- ⚠️ You must use the same license (AGPL-3.0)
- ⚠️ You must state changes made to the code

For commercial licensing options without AGPL restrictions, please contact the authors.

See the [LICENSE](LICENSE) file for the full license text.

## Acknowledgments

- Built with [Alloy](https://github.com/alloy-rs/alloy) and [REVM](https://github.com/bluealloy/revm)
- Inspired by traditional debuggers and Ethereum development tools
- Special thanks to the Foundry and Ethereum development communities