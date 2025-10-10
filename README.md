<!-- Project Banner -->
<p align="center">
  <img src="./resources/edb-banner.png" alt="EDB Banner" width="80%" />
</p>

<h1 align="center">EDB: The Ethereum Project Debugger 🔍</h1>

<p align="center">
  <strong>Source-level time-travel debugger for Ethereum smart contracts</strong>
</p>

<p align="center">
  <a href="#installation">Installation</a> •
  <a href="#quickstart">Quickstart</a> •
  <a href="#why-edb">Why EDB?</a> •
  <a href="#sponsors">Sponsors</a>
</p>

EDB bridges the gap between high-level Solidity code and low-level EVM execution, providing four essential debugging features that have been missing from the Ethereum ecosystem:

- 🧭 **Step-by-step execution at the source code level**  
- 🧠 **Local variable value inspection**  
- 🧮 **Custom expression evaluation during debug execution**
- 🎯 **Breakpoints & watchpoints for fine-grained control**


> ⚠️ **Note**: EDB is currently under active development. Features and APIs may change as we continue to improve the debugging experience.

<!-- Demo GIF with framing -->
<p align="center">
  <img src="resources/edb-demo-min.gif" alt="EDB Demo" width="100%" style="border: 1px solid #ddd; border-radius: 8px;">
</p>
<p align="center"><em>✨ Time-travel through your Solidity code with a full TUI debugger.</em></p>

## Installation

### Prerequisites

- **Ethereum RPC endpoint** - Public (like Infura/Alchemy) or local node

### One-line Install

```bash
curl -sSL https://install.edb.sh | bash
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/edb-rs/edb
cd edb

# Build all components
cargo build --release

# Install binaries
cargo install --path crates/edb
cargo install --path crates/rpc-proxy
cargo install --path crates/tui
```

## Quickstart

### Debug an On-Chain Transaction

Debug any transaction from mainnet or testnets:

```bash
# Debug a transaction with Terminal UI (default)
edb --rpc-urls <RPC_ENDPOINTS> replay 0x5bedd885ff628e935fe47dacb6065c6ac80514a85ec6444578fd1ba092904096
```
The `RPC_ENDPOINTS` should be a comma-separated list of RPC endpoint URLs.
EDB will utilize the RPC endpoints to obtain on-chain states to replay the transaction.
The more RPC endpoints are provided, the faster the replay is.
__If none is provided, EDB will default to the ten most popular public RPC endpoints, which may be slow and unreliable.__

EDB will by default start a TUI debugger:
<p align="center">
  <img src="resources/edb-tui.png" alt="EDB Demo" width="98%" style="border: 1px solid #ddd; border-radius: 8px;">
</p>

Type `?` in the TUI to view the help page.


## Why EDB?

Traditional Ethereum debugging tools operate at the bytecode level, making it nearly impossible to understand what's happening in your Solidity code.

Tools like [Remix IDE's debugger](https://remix-ide.readthedocs.io/en/latest/debugger.html), [Foundry's `forge debug`](https://book.getfoundry.sh/forge/debugger), and [Hardhat's console debugger](https://hardhat.org/hardhat-network/docs/guides/forking-other-networks) show you opcode-by-opcode execution, stack traces, and raw memory dumps.
While powerful, these tools require developers to mentally map between high-level Solidity constructs and low-level EVM operations, which is, however, a complex and error-prone process.

**The fundamental challenge:** While Solidity compilers generate source maps to link bytecode back to source code, this mapping is fragile and often imprecise, especially for optimized contracts.

Existing debuggers rely on these source maps to display which source line corresponds to each opcode, but they still can't reliably reconstruct high-level variable values, function call contexts, or complex data structures from raw EVM state.
The source maps frequently point to wrong lines or become completely unreliable when compiler optimizations are enabled.

**EDB's solution:** Instead of trying to decode bytecode back to source level, we instrument your Solidity contracts at the source code level.
By inserting strategic debugging hooks during compilation, EDB creates contracts that can report their own state in terms of your original high-level constructs.

### What makes EDB different:

- **True source-level debugging** - Step through your actual Solidity code, not disassembled bytecode
- **Reliable variable inspection** - Access any local variable, struct field, or array element with confidence
- **Expression evaluation** - Evaluate arbitrary Solidity expressions against the current execution state
- **Time-travel capabilities** - Navigate backward and forward through execution history
- **Breakpoints & watchpoints** - Set conditional and unconditional breakpoints, and watchpoints on expressions

## Sponsors

<br>
<div align="center">
  <a href="https://esp.ethereum.foundation/" target="_blank">
    <img src="https://esp.ethereum.foundation/_next/static/media/esp-logo.96fc01cc.svg" alt="Ethereum Foundation ESP" height="60" style="margin: 20px;">
  </a>
</div>
<br>

<em>Love EDB? Your support keeps us building! <a href="https://github.com/sponsors/edb-rs">Buy us a coffee ☕</a> and help us dedicate more time to making Ethereum debugging better.</em>
