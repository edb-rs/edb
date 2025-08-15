# EDB RPC Proxy

A high-performance, intelligent caching RPC proxy for Ethereum that can **reduce RPC requests by 70-90%** and dramatically accelerate blockchain debugging and development workflows.

## 🚀 Quick Start

**Install and run in 30 seconds:**

```bash
# Clone and build
git clone https://github.com/MedGa-eth/EDB
cd EDB
cargo build --release -p edb-rpc-proxy

# Start with sensible defaults
./target/release/edb-rpc-proxy

# Or with custom configuration
./target/release/edb-rpc-proxy \
  --port 8546 \
  --max-cache-items 500000 \
  --grace-period 300
```

**Use immediately:**
```bash
# Point your RPC client to the proxy
curl -X POST http://localhost:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

## 🎯 Key Benefits

- **🔥 Massive Performance Gains**: 70-90% reduction in RPC requests through intelligent caching
- **💰 Cost Savings**: Significantly reduce paid RPC service costs for teams
- **⚡ Instant Debug Traces**: Cache expensive debug/trace calls for instant subsequent access
- **🛡️ High Availability**: Automatic failover across 13+ RPC providers with health monitoring
- **🤝 Team Collaboration**: Shared cache benefits entire team's debugging sessions
- **🔧 Zero Configuration**: Works out-of-the-box with sensible defaults
- **🌐 Universal Compatibility**: Works with any Ethereum RPC client (MetaMask, Foundry, Hardhat, etc.)

## 📋 Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Installation](#installation)
- [Configuration](#configuration)
- [CLI Arguments](#cli-arguments)
- [EDB Integration](#edb-integration)
- [API Endpoints](#api-endpoints)
- [Use Cases](#use-cases)
- [Performance Tuning](#performance-tuning)
- [Development](#development)

## 📖 Overview

EDB RPC Proxy was originally developed as part of the [EDB (Ethereum Debugger)](../edb/) project but provides universal benefits for any Ethereum development workflow. While it includes EDB-specific features like instance registration and lifecycle management, **the core caching and multi-provider functionality benefits any RPC client**.

### What Makes It Special?

- **Intelligent Caching**: Understands which RPC methods are cacheable and avoids caching non-deterministic requests
- **Multi-Provider Management**: Built-in failover across multiple RPC endpoints with health monitoring
- **Production Ready**: Atomic disk persistence, graceful shutdown, comprehensive error handling
- **Highly Configurable**: Fine-tune caching, provider health, and lifecycle management

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              EDB RPC PROXY                                     │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐             │
│  │   EDB Client    │    │   Any RPC       │    │   Hardhat/      │             │
│  │   Instance      │    │   Client        │    │   Foundry       │             │
│  └─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘             │
│            │                      │                      │                     │
│            └──────────────────────┼──────────────────────┘                     │
│                                   │                                            │
│                            ┌──────▼──────┐                                     │
│                            │   HTTP API  │                                     │
│                            │  (Port 8546) │                                     │
│                            └──────┬──────┘                                     │
│                                   │                                            │
│            ┌──────────────────────┼──────────────────────┐                     │
│            │                      ▼                      │                     │
│            │         ┌─────────────────────┐             │                     │
│            │         │    RPC HANDLER      │             │                     │
│            │         │                     │             │                     │
│            │         │ ┌─────────────────┐ │             │                     │
│            │         │ │ Smart Cache     │ │             │                     │
│            │         │ │ • 55+ Methods   │ │             │                     │
│            │         │ │ • Deterministic │ │             │                     │
│            │         │ │ • Block Params  │ │             │                     │
│            │         │ └─────────────────┘ │             │                     │
│            │         └──────────┬──────────┘             │                     │
│            │                    │                        │                     │
│    ┌───────▼────────┐          │          ┌─────────▼────────┐                │
│    │  CACHE MANAGER │          │          │ PROVIDER MANAGER │                │
│    │                │          │          │                  │                │
│    │ ┌────────────┐ │          │          │ ┌──────────────┐ │                │
│    │ │ In-Memory  │ │          │          │ │Round-Robin   │ │                │
│    │ │ LRU Cache  │ │          │          │ │Load Balancer │ │                │
│    │ └────────────┘ │          │          │ └──────────────┘ │                │
│    │ ┌────────────┐ │          │          │ ┌──────────────┐ │                │
│    │ │ Disk Cache │ │          │          │ │Health Monitor│ │                │
│    │ │ Atomic I/O │ │          │          │ │Every 60s     │ │                │
│    │ └────────────┘ │          │          │ └──────────────┘ │                │
│    └────────────────┘          │          └─────────┬────────┘                │
│                                 │                    │                         │
│                          ┌──────▼──────┐             │                         │
│                          │   CACHE     │             │                         │
│                          │    HIT      │             │                         │
│                          └──────┬──────┘             │                         │
│                                 │                    │                         │
│                          ┌──────▼──────┐    ┌────────▼───────┐                 │
│                          │   Return    │    │  Forward to    │                 │
│                          │   Cached    │    │   Upstream     │                 │
│                          │  Response   │    │   Provider     │                 │
│                          └─────────────┘    └────────┬───────┘                 │
│                                                      │                         │
└─────────────────────────────────────────────────────┼─────────────────────────┘
                                                       │
                     ┌─────────────────────────────────┼─────────────────────────┐
                     │              UPSTREAM RPC PROVIDERS                      │
                     └─────────────────────────────────┼─────────────────────────┘
                                                       │
              ┌──────────────┬──────────────┬──────────▼──────────┬──────────────┐
              │              │              │                     │              │
         ┌────▼───┐    ┌─────▼────┐   ┌─────▼────┐         ┌─────▼────┐   ┌─────▼────┐
         │Gateway │    │PublicNode│   │Tenderly  │   ...   │  Ankr    │   │ LlamaRPC │
         └────────┘    └──────────┘   └──────────┘         └──────────┘   └──────────┘
```

### Core Components

1. **Smart RPC Handler**: Analyzes requests for cacheability and deterministic parameters
2. **Cache Manager**: In-memory LRU cache with atomic disk persistence and merge logic
3. **Provider Manager**: Round-robin load balancing with health monitoring across 13+ providers
4. **EDB Registry**: Lifecycle management for EDB instances (optional, benefits any client)

## 🔧 Installation

### Prerequisites

- Rust 1.88+ ([install via rustup](https://rustup.rs/))
- Git

### Build from Source

```bash
# Clone the EDB repository
git clone https://github.com/MedGa-eth/EDB
cd EDB

# Build the RPC proxy
cargo build --release -p edb-rpc-proxy

# The binary will be at: ./target/release/edb-rpc-proxy
```

### Verify Installation

```bash
./target/release/edb-rpc-proxy --help
```

## ⚙️ Configuration

### Basic Usage

```bash
# Start with defaults (recommended for most users)
edb-rpc-proxy

# Start with custom port
edb-rpc-proxy --port 8547

# Use custom RPC providers
edb-rpc-proxy --rpc-urls "https://mainnet.infura.io/v3/YOUR_KEY,https://eth.llamarpc.com"
```

### Configuration Examples

**Development Mode** (auto-shutdown when idle):
```bash
edb-rpc-proxy --grace-period 300 --cache-save-interval 1
# Saves cache every minute, shuts down after 5 minutes of no activity
```

**Production Mode** (long-running service):
```bash
edb-rpc-proxy --port 8546 --max-cache-items 500000 --grace-period 0
# Large cache, never auto-shutdown, standard port
```

**High-Performance Setup**:
```bash
edb-rpc-proxy \
  --max-cache-items 1000000 \
  --cache-save-interval 10 \
  --health-check-interval 30 \
  --max-failures 2
# 1M item cache, frequent health checks, fast failover
```

## 📋 CLI Arguments

### General Configuration

| Argument | Default | Description |
|----------|---------|-------------|
| `--port` | `8546` | HTTP server listening port |
| `--rpc-urls` | *13 public RPCs* | Comma-separated upstream RPC endpoints |

### Cache Configuration

| Argument | Default | Description |
|----------|---------|-------------|
| `--max-cache-items` | `102400` | Maximum cached responses (~100MB for 100k items) |
| `--cache-dir` | `~/.edb/cache/rpc/<chain_id>/` | Cache storage directory |
| `--cache-save-interval` | `5` | Minutes between disk saves (0 = shutdown only) |

### Provider Health

| Argument | Default | Description |
|----------|---------|-------------|
| `--max-failures` | `3` | Failures before marking provider unhealthy |
| `--health-check-interval` | `60` | Seconds between provider health checks |

### EDB Integration (Optional)

| Argument | Default | Description |
|----------|---------|-------------|
| `--grace-period` | `0` | Seconds before auto-shutdown when no EDB instances (0 = never) |
| `--heartbeat-interval` | `10` | Seconds between EDB instance health checks |

### Resource Usage Estimates

- **Memory**: `50MB + (max_cache_items × 1KB)`
- **Disk**: Cache size varies by response complexity
- **Network**: Reduced by 70-90% due to caching

## 🔌 EDB Integration

While the proxy works with any RPC client, it includes special features for EDB instances:

### Instance Registration

EDB instances can register themselves for lifecycle management:

```bash
# EDB instances automatically call:
curl -X POST http://localhost:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"edb_register","params":[12345, 1703123456],"id":1}'
```

### Heartbeat Monitoring

Registered instances send periodic heartbeats:

```bash
curl -X POST http://localhost:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"edb_heartbeat","params":[12345],"id":1}'
```

### Auto-Shutdown

When `--grace-period > 0`, the proxy automatically shuts down when no EDB instances are active, preventing resource waste.

**This functionality can be adopted by any project** - simply implement the registration and heartbeat calls in your application.

## 📡 API Endpoints

### Standard JSON-RPC

All standard Ethereum RPC methods are supported and automatically cached when appropriate:

```bash
curl -X POST http://localhost:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0x1000000",false],"id":1}'
```

### Management Endpoints

| Method | Description | Example |
|--------|-------------|---------|
| `edb_ping` | Service health check | Returns status and timestamp |
| `edb_info` | Detailed service info | Version, uptime, PID |
| `edb_cache_stats` | Cache utilization | Hit rate, size, utilization |
| `edb_active_instances` | List registered EDB instances | PIDs of active instances |
| `edb_providers` | Provider health status | Health, response times, failures |
| `edb_shutdown` | Graceful shutdown | Saves cache and stops service |

### Example Management Calls

```bash
# Check service health
curl -X POST http://localhost:8546 \
  -d '{"jsonrpc":"2.0","method":"edb_ping","id":1}'

# Get cache statistics
curl -X POST http://localhost:8546 \
  -d '{"jsonrpc":"2.0","method":"edb_cache_stats","id":1}'

# Check provider health
curl -X POST http://localhost:8546 \
  -d '{"jsonrpc":"2.0","method":"edb_providers","id":1}'
```

## 🎯 Use Cases

### 1. Blockchain Debugging Teams

**Problem**: Debug traces are expensive to compute but teams often debug the same transactions.

**Solution**: First developer pays the computation cost, subsequent team members get instant responses.

```bash
# First call: ~2 seconds, hits upstream RPC
curl -X POST http://localhost:8546 \
  -d '{"jsonrpc":"2.0","method":"debug_traceTransaction","params":["0x123..."],"id":1}'

# Subsequent calls: ~10ms, served from cache
# Same call by any team member gets instant response
```

### 2. Development Workflow Optimization

**Problem**: Hardhat/Foundry tests repeatedly query the same block data.

**Solution**: Cache immutable blockchain data across test runs.

```bash
# Configure your hardhat.config.js or foundry.toml to use the proxy
networks: {
  mainnet: {
    url: "http://localhost:8546",  // Instead of direct RPC
    // ... other config
  }
}
```

### 3. RPC Cost Reduction

**Problem**: Paid RPC services charge per request, costs accumulate quickly.

**Solution**: 70-90% reduction in upstream requests through intelligent caching.

```bash
# Use paid RPC as upstream, proxy provides caching layer
edb-rpc-proxy --rpc-urls "https://mainnet.infura.io/v3/YOUR_KEY"
```

### 4. RPC Reliability

**Problem**: Single RPC endpoint creates a point of failure.

**Solution**: Automatic failover across multiple providers with health monitoring.

```bash
# Proxy automatically handles provider failures
edb-rpc-proxy --max-failures 2 --health-check-interval 30
```

## 🚀 Performance Tuning

### Cache Optimization

```bash
# For high-volume usage, increase cache size
edb-rpc-proxy --max-cache-items 1000000

# For memory-constrained environments
edb-rpc-proxy --max-cache-items 50000
```

### Provider Optimization

```bash
# Faster failover for critical applications
edb-rpc-proxy --max-failures 1 --health-check-interval 15

# More tolerance for unstable providers
edb-rpc-proxy --max-failures 5 --health-check-interval 120
```

### Persistence Tuning

```bash
# Frequent saves for critical data
edb-rpc-proxy --cache-save-interval 1

# Reduce I/O for high-performance setups
edb-rpc-proxy --cache-save-interval 30
```

## 🔍 Cached Methods

The proxy intelligently caches 55+ RPC methods when they use deterministic parameters:

### Always Cached
- **Chain constants**: `eth_chainId`, `net_version`
- **Transaction data**: `eth_getTransactionByHash`, `eth_getTransactionReceipt`
- **Block data**: `eth_getBlockByHash`, `eth_getBlockByNumber` (with specific block numbers)

### Conditionally Cached
- **State queries**: `eth_call`, `eth_getBalance`, `eth_getCode` (only with specific block numbers, not "latest")
- **Debug traces**: `debug_traceTransaction`, `trace_transaction` (immutable once computed)
- **Logs**: `eth_getLogs` (only with specific block ranges)

### Never Cached
- **Dynamic data**: `eth_blockNumber`, `eth_gasPrice`, `eth_estimateGas`
- **Account state**: Methods using "latest", "pending", "safe", "finalized" block parameters
- **Network state**: `net_peerCount`, `eth_syncing`

## 🛠️ Development

### Running from Source

```bash
# Development mode with debug logging
RUST_LOG=debug cargo run --bin edb-rpc-proxy -- --grace-period 300

# Run tests
cargo test -p edb-rpc-proxy

# Run with custom configuration
cargo run --bin edb-rpc-proxy -- \
  --port 8547 \
  --max-cache-items 10000 \
  --cache-save-interval 1
```

### Testing

```bash
# Run all tests
cargo test -p edb-rpc-proxy

# Run specific test
cargo test -p edb-rpc-proxy test_cache_behavior

# Run with output
cargo test -p edb-rpc-proxy -- --nocapture
```

### Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make changes and add tests
4. Run tests: `cargo test -p edb-rpc-proxy`
5. Submit a pull request

## 📝 License

This project is licensed under the MIT OR Apache-2.0 license.

## 🤝 Community

- **Issues**: [GitHub Issues](https://github.com/MedGa-eth/EDB/issues)
- **Discussions**: [GitHub Discussions](https://github.com/MedGa-eth/EDB/discussions)

## 🙏 Acknowledgments

- Built as part of the [EDB (Ethereum Debugger)](../edb/) project
- Uses public RPC endpoints from various providers
- Inspired by the need for efficient blockchain development workflows

---

**Get started in 30 seconds** → Just run `edb-rpc-proxy` and point your RPC client to `http://localhost:8546`!