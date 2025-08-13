## Rust-Based Ethereum Debugger

This project builds a **step-by-step debugger for Ethereum transactions** in Rust.
The primary dependencies are **`alloy`** and **`revm`**; it may also use **`foundry-compilers`** and **`foundry-block-explorers`**. For current versions, see Foundry's workspace [`Cargo.toml`](https://github.com/foundry-rs/foundry/blob/master/Cargo.toml).

The workspace will ultimately contain:

| Crate                              | Purpose                                    |
| ---------------------------------- | ------------------------------------------ |
| **`engine`** (currently `backend`) | Core analysis and instrumentation logic    |
| **`tui`**                          | Terminal user interface front-end          |
| **`webui`**                        | Web user interface front-end               |
| **`rpc-proxy`**                    | Caching RPC proxy server                   |
| **`edb`** (binary)                 | Orchestrator that ties everything together |

---

### Dependency Management

All dependencies used in EDB should be kept synchronized with [Foundry's Cargo.toml](https://github.com/foundry-rs/foundry/blob/master/Cargo.toml) to ensure compatibility and leverage the latest improvements from the Foundry ecosystem.

**Key principles:**

1. **Version Alignment** – Use the exact same versions of core dependencies (`alloy`, `revm`, `foundry-*` crates) as specified in Foundry's workspace Cargo.toml.

2. **Regular Updates** – Periodically check Foundry's repository for dependency updates and align EDB's dependencies accordingly.

3. **Compatibility Testing** – When updating dependencies, ensure all components (engine, tui, webui, rpc-proxy) remain compatible.

4. **Rationale** – Staying aligned with Foundry ensures:
   * Access to the latest Ethereum protocol updates
   * Compatibility with Foundry's toolchain
   * Benefit from security patches and performance improvements
   * Consistent behavior with other Ethereum development tools

---

### RPC Proxy Server `rpc-proxy`

`rpc-proxy` acts as an intelligent caching layer between EDB components and real Ethereum RPC endpoints. It provides real-time caching capabilities to support multiple concurrent debugging sessions efficiently.

**Key Features:**

1. **Transparent Proxy** – Acts as a drop-in replacement for direct RPC connections, requiring no changes to existing code.

2. **Smart Caching** – Caches immutable and deterministic RPC responses to reduce network overhead and improve performance.

3. **Real-time Cache Sharing** – Multiple EDB debugging instances can share cached data in real-time, improving efficiency when debugging related transactions.

4. **Automatic Process Management** – Spawned automatically by the `edb` binary when starting a debugging session.

**Cacheable RPC Methods:**

The following RPC methods are cached as they return immutable or block-deterministic data:

* `eth_getCode` – Contract bytecode at a specific block
* `eth_getStorageAt` – Storage slot values at a specific block
* `eth_getTransactionByHash` – Transaction details (immutable once mined)
* `eth_getRawTransactionByHash` – Raw transaction data
* `eth_getTransactionReceipt` – Transaction receipts (immutable once mined)
* `eth_getBlockByNumber` – Block information (immutable once finalized)
* `eth_getBlockByHash` – Block information by hash
* `eth_getLogs` – Event logs for specific filters
* `eth_getProof` – Merkle proofs at specific blocks
* `eth_getBlockReceipts` – All receipts in a block

**Cache Storage:**

The proxy server uses the existing `EDBCache` infrastructure from `utils/src/cache.rs`:
* Cache location: `~/.edb/cache/rpc/<chain_id>/`
* Configurable TTL for different data types
* Automatic cache invalidation for expired entries
* Maximum cache size management to prevent unbounded growth

**Integration with EDB:**

1. When `edb` starts, it spawns the `rpc-proxy` server as a detached independent process on a local port (default: 8546)
2. The proxy server connects to the upstream RPC endpoint specified by `--rpc-url`
3. All EDB components connect to the proxy instead of the real RPC endpoint
4. The proxy transparently handles caching and forwarding of requests
5. The proxy process survives EDB crashes/kills and can serve multiple EDB instances simultaneously

**Proxy Lifecycle Management:**

To enable efficient sharing between multiple EDB instances while avoiding resource conflicts:

1. **Shared Proxy Discovery:**
   - Each RPC endpoint + chain combination uses a single shared proxy instance
   - EDB startup checks if compatible proxy already exists on expected port
   - Uses quick TCP connection test followed by health check RPC

2. **Health Check Protocol:**
   - `edb_ping` – Custom RPC method for proxy health verification
   - `edb_info` – Returns proxy metadata (version, uptime, cache statistics)
   - Standard JSON-RPC errors indicate proxy failure or incompatibility

3. **Instance Registration:**
   - Active EDB instances register with proxy using process ID and timestamp
   - Proxy maintains registry of connected EDB instances
   - Periodic heartbeat mechanism detects dead EDB processes

4. **Automatic Startup/Shutdown:**
   - If no healthy proxy exists, EDB spawns new proxy instance using detached process spawning
   - Uses platform-specific detachment: `process_group(0)` on Unix/macOS, `DETACHED_PROCESS` on Windows
   - Port conflict resolution tries alternative ports automatically
   - Proxy enters 30-second grace period when last EDB instance disconnects
   - Proxy shuts down only after grace period expires with no new connections

5. **Graceful Handoff:**
   - New EDB instances can connect to existing proxy during grace period
   - Prevents unnecessary proxy restarts for quick EDB session changes
   - Cache data preserved across EDB instance lifecycles

**Configuration:**

* `--proxy-port <number>` – Port for the RPC proxy server (default: 8546)
* `--cache-ttl <seconds>` – Cache time-to-live (default varies by data type)
* `--max-cache-items <number>` – Maximum number of cached items (default: 102400)
* `--disable-cache` – Bypass caching for all requests
* `--proxy-grace-period <seconds>` – Grace period before proxy shutdown (default: 30)
* `--force-new-proxy` – Always start a new proxy instance instead of reusing existing
* `--proxy-heartbeat-interval <seconds>` – Heartbeat interval for EDB instance registration (default: 10)

---

### Stand-Alone Binary `edb`

`edb` is the entry point and supports two modes:

1. **Replay an existing transaction** – the user supplies a transaction hash.
2. **Debug a Foundry test case** – the user supplies a test name; `edb` locates the corresponding transaction.

Key CLI flags:

* **`--rpc-url`** – Ethereum RPC endpoint.
* **`--ui {tui|web}`** – choose the TUI (`tui` crate) or Web UI (`webui` crate).
* **`--block <number>`** – override automatic block number detection.
* **`--port <number>`** – specify the port for the JSON-RPC server (default: 8545).
* **`--proxy-port <number>`** – specify the port for the RPC proxy server (default: 8546).
* **`--disable-cache`** – disable RPC caching and connect directly to the upstream RPC.

`edb` also reads environment variables for provider and explorer API keys.

Workflow:

1. Check for existing RPC proxy server:
   - Test connection to expected proxy port
   - Verify proxy health with `edb_ping` RPC call
   - Register with existing proxy or start new instance if needed

2. Fork the chain at the correct block through the proxy, replaying earlier transactions in that block. The proxy caches all fetched data for potential reuse.

3. Build three inputs for the engine:
   * **Forked database** – ready to replay the target transaction.
   * **Env with Handler Cfg** – execution-environment settings.
   * **Port number** – port number for the JSON-RPC which we will discuss later.

4. Call `engine::analyze`.

5. After analysis, launch the selected UI (TUI directly; Web UI via a browser prompt) to connect to the JSON-RPC server hosted by `engine`.

---

### Core Engine `engine`

`engine` exposes `analyze`, which:

1. **Replays** the target transaction to collect all touched contract addresses.

2. **Aborts** if the transaction reverts due to gas issues (other reverts are acceptable).

3. **Downloads verified source code** for each contract from Etherscan (see Foundry's [clone.rs](https://github.com/foundry-rs/foundry/blob/409745944204d6e0fa12474238a76618c3899ba7/crates/forge/src/cmd/clone.rs)).

4. **Analyzes and instruments** each source file (largely interfaces only for now, except the precompile call mentioned later). Inserts a call to a precompile at address `0x000…023333` (the exact value is arbitrary) at the start of every function.

5. **Recompiles** the instrumented contracts and **redeploys** them at their original block height. To do this correctly, you must reconstruct the deployment transaction using both the compiled deployment code and the constructor arguments from the original deployment transaction. If needed, replay prior same-block transactions before deployment to maintain consistency. This process produces the modified bytecode.

6. **Replaces** the original bytecode in the forked database with the instrumented versions.

7. **Re-executes** the transaction. On each injected precompile call, create a new fork and eventually produce a chain of state-snapshot databases.

8. **Hosts a JSON-RPC server** (stub only) so front-ends can inspect and control sessions.

---

### Terminal UI `tui`

`tui` communicates with the engine’s RPC server to display status and drive interaction.
For now, generate only skeleton interfaces; leave implementations empty.

---

### Web UI `webui`

`webui` will provide a browser-based interface, also speaking to the engine's RPC server.
As with `tui`, create only placeholders and public interfaces for now.
