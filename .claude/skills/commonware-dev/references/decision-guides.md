# Primitive Selection Guides

> Decision frameworks for choosing the right commonware primitives for your use case.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Consensus variants | `search_code(query="pub struct Engine", crate="commonware-consensus", version=PINNED)` |
| Storage types | `search_code(query="pub trait", crate="commonware-storage", version=PINNED)` |
| P2P config presets | `search_code(query="Config::recommended", crate="commonware-p2p", version=PINNED)` |
| Crypto scheme traits | `search_code(query="pub trait Signer", crate="commonware-cryptography", version=PINNED)` |
| Parallel strategies | `search_code(query="pub trait Strategy", crate="commonware-parallel", version=PINNED)` |

## Choosing a Consensus Approach

### When to Use Simplex

Use `commonware-consensus::simplex` when:
- You need BFT consensus with known validator set
- Message complexity matters (Simplex is O(n) per round)
- You want simple, auditable consensus logic
- Validators have roughly equal stake/weight

**Trade-offs**:
- Requires 2/3+ honest validators
- Synchrony assumptions for liveness
- Single leader per view (bottleneck for high throughput)

### When to Use Threshold Simplex

Use `commonware-consensus::threshold_simplex` when:
- You need aggregate signatures on certificates
- Validator weights vary significantly
- You want smaller certificate sizes (one aggregate sig vs n sigs)
- You're doing committee-based consensus

**Trade-offs**:
- Requires BLS12-381 (more expensive than Ed25519)
- DKG setup required for threshold keys
- More complex key management

### When to Use Aggregation

Use `commonware-consensus::aggregation` when:
- You need epoch-based certificate aggregation
- Running parallel with view-based consensus
- Building systems with periodic checkpoints

**Trade-offs**:
- Additional complexity vs single consensus
- Requires coordination between epoch and view supervisors

## Choosing a Storage Primitive

### Quick Selection Table

| Use Case | Primitive | Why |
|----------|-----------|-----|
| Append-only log with replay | **Journal** | Crash recovery, sequential access, pruning |
| Mutable cursors / small state | **Metadata** | Atomic batch commit, crash-safe, mutable KV |
| Write-once records by index + key | **Archive** | Immutable, dual lookup, prunable variant available |
| Write-once records by key (large dataset) | **Freezer** | Minimal memory, zero write amplification |
| Sequential cache with gap detection | **Cache** | Index-only lookup, compression, pruning |
| Sparse index → fixed-size value | **Ordinal** | O(1) positional access, out-of-order insert |
| Full CRUD with Merkle proofs | **QMDB** | Authenticated state, state sync (ALPHA) |
| Merkle proofs for append-only data | **MMR** | Efficient inclusion proofs |
| Merkle proofs for a batch of items | **BMT** | Stateless tree, single + multi proofs |

> **Note:** `commonware-storage::kv` defines abstract traits (`Gettable`, `Updatable`, `Deletable`, `Batchable`) — not a concrete store. The concrete implementations are the primitives listed above.

### Journal

**Best for**: Consensus logs, transaction logs, WAL (write-ahead logs), event sourcing

Use when: data is append-only, you need crash recovery, you need sequential replay, and order matters. Available in contiguous (position-based) and segmented (section-based) variants, each with fixed-size and variable-size item support. Avoid when: you need random access by key (use Archive or Metadata), or need to update existing entries.

### Metadata

**Best for**: Watermarks, cursors, epoch state, small mutable configuration

Use when: you need a small collection of mutable key-value pairs with crash-safe atomic batch writes. Supports `put`, `get`, `get_mut`, `remove`, `retain`, `clear`. Uses dual-blob atomic writes with CRC32 checksums. Delta writes optimize for stable key sets. Avoid when: you have many thousands of keys, need append-only semantics, or need Merkle proofs.

### Archive

**Best for**: Finalized blocks, certificates, immutable records

Use when: each record is written exactly once and never updated. Lookup by `index` (u64) or `key`. `put` is idempotent — does nothing if the index already exists. Available as `immutable` (Freezer-backed, minimal memory) or `prunable` (Journal + Ordinal + Index, supports old data reclamation). Avoid when: you need to update records after creation (e.g., status transitions). Archive is strictly write-once.

### Merkle Mountain Range (MMR)

**Best for**: Light client proofs, commitment schemes, verifiable logs

Use when: you need efficient append + proof generation, proving inclusion of historical data, or building light client protocols. Avoid when: you need to update existing entries, need key-based lookups, or data isn't strictly append-only.

### Storage Composition Patterns

Individual storage primitives are rarely used alone. Choose a composition based on your use case:

| Use Case | Storage Primitives | Example |
|----------|-------------------|---------|
| Simple consensus app | `storage-dir` (automatic via runtime) | log |
| Production blockchain | Archives (finalized blocks + certs) | bridge |
| DKG/ceremony state | Journals (events) + Metadata (epoch state) | reshare |
| Idempotent cross-chain actions | Journal (action records) + Metadata (scan watermarks) | asula_btc_bridge |
| Verifiable database sync | QMDB + MMR (proofs) | sync |
| Stress testing / ephemeral | In-memory only (no persistence) | chat, estimator, flood |

**DKG storage pattern** (reshare):
- **Journal** for event persistence (dealings, acks, logs) — enables crash recovery via replay
- **Metadata** for epoch state (round, output, share) — key-value for current state
- Replay on startup rebuilds in-memory caches from journal
- Prune old epochs: retain only current + next

**Production consensus pattern** (bridge):
- **Archive** for finalized blocks (immutable, compressed with zstd)
- **Archive** for finalization certificates (indexed by height)
- **Consensus engine** manages its own WAL via `storage-dir`

## Choosing a Cryptographic Scheme

### Ed25519 vs BLS12-381

| Criterion | Ed25519 | BLS12-381 |
|-----------|---------|-----------|
| Signature size | 64 bytes | 48 bytes |
| Public key size | 32 bytes | 96 bytes |
| Signing speed | Faster | Slower |
| Verification speed | Faster | Slower |
| Aggregation | No | Yes |
| Threshold signatures | Complex | Native |
| Batch verification | Yes | Yes |

**Choose Ed25519 when**:
- Individual signatures (no aggregation needed)
- Performance is critical
- Simple key management
- P2P authentication

**Choose BLS12-381 when**:
- Aggregating many signatures (certificates)
- Threshold cryptography (DKG, resharing)
- Minimizing on-chain signature storage
- Committee-based protocols

### Hash Function Selection

| Function | Use Case |
|----------|----------|
| **SHA256** | Block hashes, Merkle trees, general cryptographic hashing |
| **Blake3** | High-performance hashing, parallel processing |
| **CRC32** | Checksums, non-cryptographic integrity |

## Choosing a Network Primitive

### Stream vs P2P Network

This is a fundamental architecture decision:

| Criterion | `commonware-stream` | `commonware-p2p` |
|-----------|--------------------|--------------------|
| Topology | Point-to-point (1:1) | Many-to-many mesh |
| Discovery | Manual dial/listen | Gossip-based discovery |
| Connection mgmt | Manual | Automatic |
| Use case | Indexer, sync server, dedicated service | Validator networks, consensus |

**Choose Stream when**:
- Communicating with a dedicated service (indexer, sync server)
- Client-server architecture (one client, one server)
- Need simple request-response over an encrypted channel

**Choose P2P Network when**:
- Multiple peers need to communicate
- Need automatic peer discovery and reconnection
- Running consensus with message routing to channels

**Can combine both**: The bridge example uses P2P for validator consensus AND stream for indexer communication in the same application.

### Authenticated vs Lookup Network

| Feature | Authenticated | Lookup |
|---------|--------------|--------|
| Discovery | Manual/config | DHT-based |
| Connections | Persistent | On-demand |
| Use case | Known validators | Dynamic peers |
| Setup | Simpler | More complex |

**Choose Authenticated when**:
- Validator set is known ahead of time
- Network topology is stable
- Running permissioned consensus

**Choose Lookup when**:
- Peers join/leave dynamically
- Need DHT-based discovery
- Building permissionless systems

### Simulated vs Real Network

| Variant | Runtime | Use case |
|---------|---------|----------|
| `simulated::Network` | Deterministic | Tests, protocol estimation |
| `authenticated::discovery::Network` | Tokio | Production |

The simulated network supports bandwidth limiting (`oracle.limit_bandwidth()`) and directional link configuration with real-world latency data, making it suitable for protocol performance estimation.

### P2P Configuration Presets

Three configuration constructors are available: `Config::local(...)` for fast local development with relaxed rate limits and localhost, `Config::recommended(...)` for production with strict rate limits and proper timeouts, and `Config::test(...)` for deterministic tests with aggressive settings. Fetch the full signatures: `search_code(query="Config::recommended", crate="commonware-p2p", version=PINNED)`.

### Channel Design Patterns

#### Standard Channel Allocation for Consensus

| Channel | ID | Purpose | Rate Limit |
|---------|-----|---------|------------|
| votes | 0 | Vote messages | 10-128/sec |
| certificates | 1 | Certificate exchange | 10-128/sec |
| resolver | 2 | Block fetching | 10-128/sec |
| broadcaster | 3 | Consensus broadcast | 8/sec |
| marshal | 4 | State machine sync | 8/sec |
| dkg | 5 | Key generation (if needed) | 128/sec |

Typical backlog: 256 messages in flight per channel.

**Rate limiting tiers**: High-frequency (128/sec) for votes, certificates, resolver, DKG. Low-frequency (8-10/sec) for broadcast, marshal, consensus.

**Separate channels when**:
- Messages have different priorities
- Rate limiting requirements differ
- Isolation needed for DoS protection

### When to Use Muxers (Epoch-Based Multiplexing)

Use muxers when running **multiple consensus engines simultaneously** (e.g., during epoch transitions). Each epoch gets independent logical channels over the same physical P2P channel. This is required for systems with dynamic validator sets where old and new epochs overlap during transitions.

## Choosing Parallelism Strategy

### Sequential vs Rayon

| Context | Strategy | Why |
|---------|----------|-----|
| Tests | **Sequential** | Determinism, reproducibility |
| Production | **Rayon** | Performance, CPU utilization |
| Debugging | **Sequential** | Easier to trace |
| Benchmarks | **Rayon** | Realistic performance |

The standard pattern is to parameterize functions with a `P: Strategy` bound so tests can pass `Sequential` and production can pass `Rayon::default()`. See parallel.md for details.

## Architectural Decision Matrix

### Building a Simple Blockchain

Consensus: simplex (fixed validators). Crypto: Ed25519 (P2P) + BLS12-381 (certificates). Storage: Journal (blocks) + Archive (finalized). Network: authenticated (known peers).

### Building with Dynamic Validators

Consensus: threshold_simplex + epoch manager. Crypto: BLS12-381 (DKG + threshold sigs). Storage: Journal + MMR (proofs for light clients). Network: lookup (dynamic discovery).

### Building a Game/Application

Consensus: threshold_simplex + aggregation. Crypto: BLS12-381 + TLE (encryption). Storage: ADB (state) + MMR (proofs) + Ordinal (seeds). Network: Multiple channels (mempool, consensus, sync).
