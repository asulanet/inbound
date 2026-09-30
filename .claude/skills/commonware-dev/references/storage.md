# Storage Primitive Reference

> **Crate:** `commonware-storage`

Persistent data storage with crash recovery, Merkle proofs, and data structures optimized for blockchain and distributed systems use cases. Stability varies by module — see the stability annotations below.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Crate-level modules and traits | `get_file(path="storage/src/lib.rs", version=PINNED)` |
| Module structure | `list_files(crate="commonware-storage", version=PINNED)` |
| Journal API (contiguous) | `get_file(path="storage/src/journal/contiguous/mod.rs", version=PINNED)` |
| Journal API (segmented) | `get_file(path="storage/src/journal/segmented/fixed.rs", version=PINNED)` |
| Metadata API | `get_file(path="storage/src/metadata/mod.rs", version=PINNED)` |
| Archive trait | `get_file(path="storage/src/archive/mod.rs", version=PINNED)` |
| Ordinal API | `get_file(path="storage/src/ordinal/mod.rs", version=PINNED)` |
| Freezer API | `get_file(path="storage/src/freezer/mod.rs", version=PINNED)` |
| Cache API | `get_file(path="storage/src/cache/mod.rs", version=PINNED)` |
| KV traits | `get_file(path="storage/src/kv/mod.rs", version=PINNED)` |
| Index (cursor, ordered/unordered) | `get_file(path="storage/src/index/mod.rs", version=PINNED, start_line=0, end_line=100)` |
| MMR API | `get_file(path="storage/src/mmr/mod.rs", version=PINNED, start_line=0, end_line=80)` |
| BMT API | `get_file(path="storage/src/bmt/mod.rs", version=PINNED, start_line=0, end_line=70)` |
| QMDB (authenticated DB) | `get_file(path="storage/src/qmdb/mod.rs", version=PINNED, start_line=0, end_line=80)` |
| Persistable trait | `get_file(path="storage/src/lib.rs", version=PINNED)` — lines 36–58 |
| Tests for patterns | `search_code(query="#[test]", crate="commonware-storage", version=PINNED)` |

**Tip:** Many storage modules have large source files due to inline tests. Always use `start_line`/`end_line` when fetching index, mmr, or qmdb modules to avoid exceeding output limits.

## The `Persistable` Trait

All durable storage primitives share the `Persistable` trait (`storage/src/lib.rs`, BETA):

- **`commit()`** — durably persist, guaranteeing survival across crash. Recovery may be needed on startup. Defaults to calling `sync()`.
- **`sync()`** — durably persist with a stronger guarantee: no recovery needed on startup. May be slower than `commit()`.
- **`destroy()`** — consume the instance and delete all persisted data.

**When to use which:** Use `sync()` for write-ahead logging (WAL) patterns where you must guarantee state is fully durable before proceeding (e.g., persisting a BTC payout tx before broadcast). Use `commit()` when you can tolerate replay-based recovery on startup.

Implementors: `journal::contiguous::fixed::Journal`, `journal::contiguous::variable::Journal`, `journal::authenticated::Journal`, `Ordinal`, `Freezer`, QMDB databases. Note: `Metadata` has its own `sync()` method but does not implement `Persistable`.

## Storage Primitives by Category

### Mutable State (supports updates)

#### Metadata (BETA)

A key-value store optimized for atomically committing a **small collection** of metadata. Ideal for cursors, watermarks, epoch state, and configuration that changes over time.

- **Mutability:** Full — `put(key, value)`, `get(&key)`, `get_mut(&key)`, `remove(&key)`, `retain()`, `clear()`, `keys()` iterator.
- **Durability:** Dual-blob atomic writes. Maintains "left" and "right" blobs with version numbers. Writes go to the older blob first, ensuring crash safety. CRC32 checksums detect partial writes.
- **Delta writes:** If key set and value lengths are stable, only writes changed bytes (not the full blob). Efficient for frequently-updated metadata with rare key additions.
- **Recovery:** On restart, reads both blobs and picks the one with the higher valid version. Tolerates single-blob corruption gracefully.
- **Use when:** Tracking watermarks/cursors (e.g., `btc.confirmed_scan_watermark`), small state snapshots, epoch metadata, anything that needs mutable KV with crash-safe atomic batches.
- **Avoid when:** You have many thousands of keys (designed for "small collections"), need Merkle proofs, or need append-only semantics.
- **MCP entry:** `get_file(path="storage/src/metadata/mod.rs", version=PINNED)` — includes usage example and full test suite.

#### QMDB — Quick Merkle Database (ALPHA)

An authenticated database with full CRUD, Merkle proofs, and state sync. Inspired by the QMDB paper. Supports ordered and unordered keyed variants plus a keyless variant.

- **Mutability:** Full — implements `kv::Gettable`, `kv::Deletable`, `kv::Batchable` in mutable state.
- **State machine:** Four states based on merkleization (Merkleized/Unmerkleized) and durability (Durable/NonDurable). Transitions via `into_mutable()`, `into_merkleized()`, `commit()`.
- **Use when:** Building stateful applications needing authenticated state with Merkle proofs, state sync between nodes, or verifiable databases.
- **Avoid when:** You just need simple KV storage — QMDB is complex. Stability is ALPHA.
- **MCP entry:** `get_file(path="storage/src/qmdb/mod.rs", version=PINNED, start_line=0, end_line=80)`.

### Write-Once / Append-Only

#### Journal (BETA)

Append-only log for persisting sequential data with crash recovery and streaming replay. The foundational building block — used internally by Archive, Cache, Freezer, and QMDB.

Two organizational models:

- **Contiguous** (`journal::contiguous`) — position-based access (0-indexed). Items stored contiguously, accessed by position. Supports `append()`, `read(position)`, `replay(start_pos, buffer)` for streaming, `prune(min_section)`, `rewind()`. Available in `fixed` (fixed-size items, `CodecFixedShared`) and `variable` (variable-size items, `CodecShared`) variants. Implements `Persistable`.
- **Segmented** (`journal::segmented`) — section-based storage where each section maps to a separate blob. Available in `fixed`, `variable`, `glob` (for oversized records), and `oversized` (coordinates crash recovery across paired journals) variants.

**Authenticated Journal** (`journal::authenticated`, ALPHA) — wraps a contiguous journal with an MMR. Item at index i corresponds to MMR leaf at location i. Enables efficient inclusion proofs for journal entries.

- **Use when:** Event logs, transaction logs, WAL, consensus logs, any sequential append + replay workload. Replay on startup rebuilds in-memory state.
- **Avoid when:** You need key-based random access (use Archive or Metadata instead), or need to update existing entries.
- **MCP entry:** `get_file(path="storage/src/journal/contiguous/mod.rs", version=PINNED)` for traits; `get_file(path="storage/src/journal/contiguous/fixed.rs", version=PINNED, start_line=60, end_line=130)` for the fixed journal struct and config.

#### Archive (BETA)

Write-once key-value store for ordered data. Each item has both a unique `index` (u64) and a unique `key`. Lookup by either. If an index already exists, `put` does nothing (idempotent).

Two variants:
- **Immutable** (`archive::immutable`) — backed by Freezer + Metadata. Minimal memory, disk-only lookups.
- **Prunable** (`archive::prunable`) — backed by Journal + Ordinal + Index. Supports `prune(min_index)` to reclaim old data.

- **API:** `put(index, key, value)`, `put_sync(index, key, value)`, `get(Identifier::Index(n))`, `get(Identifier::Key(&k))`, `has(...)`, `next_gap(index)`, `missing_items(index, max)`, `ranges()`, `sync()`, `destroy()`.
- **Use when:** Finalized blocks, certificates, immutable records where each entry is written exactly once and never updated.
- **Avoid when:** You need to update records after creation (e.g., status transitions on deposit records). Archive is strictly write-once.
- **MCP entry:** `get_file(path="storage/src/archive/mod.rs", version=PINNED)`.

#### Freezer (BETA)

Immutable key-value store optimized for minimal memory usage and zero write amplification. Data is written once and never modified or compacted. Uses a three-level architecture: extendible hash table + key index journal + value journal.

- **Performance:** Low latency for recent data, logarithmic increase for old data.
- **Use when:** Permanent storage of large datasets with key-based lookups, resource-constrained environments.
- **Avoid when:** You need updates, deletions, or pruning. Used internally by Archive (immutable variant).
- **MCP entry:** `get_file(path="storage/src/freezer/mod.rs", version=PINNED, start_line=0, end_line=50)`.

#### Cache (ALPHA)

Prunable cache for ordered data with index-based lookups only (no key-based access). Backed by a segmented variable journal with an in-memory location index (~16 bytes overhead per item). Supports optional zstd compression.

- **API:** `put(index, value)`, `get(index)`, `next_gap(index)`, `prune(min_index)`, `sync()`.
- **Use when:** Caching sequential data (blocks, messages) where you know the exact index. Simpler than Archive when you don't need key-based lookups.
- **MCP entry:** `get_file(path="storage/src/cache/mod.rs", version=PINNED, start_line=0, end_line=80)`.

### Index & Mapping Structures

#### Index (BETA)

Memory-efficient in-memory index mapping translated keys to values. Keys are compressed via a `Translator` into fixed-size representations. Supports insert, remove, update, and cursor-based iteration.

Three variants: `ordered` (maintains insertion order), `unordered` (hash-map style), `partitioned` (sharded ordered/unordered). Used internally by Archive (prunable) and QMDB.

- **Warning:** Translator collisions are possible — `get` returns all values for a translated key.
- **Use when:** Building custom storage compositions needing key→value mapping with minimal memory. Rarely used directly.
- **MCP entry:** `get_file(path="storage/src/index/mod.rs", version=PINNED, start_line=0, end_line=100)`.

#### Ordinal (BETA)

Persistent index mapping sparse u64 indices to fixed-size `Array` values. File position corresponds directly to index (O(1) read/write). Supports out-of-order insertion. Uses RMap internally for gap tracking.

- **API:** `put(index, value)`, `get(index)`, `has(index)`, `next_gap(index)`, `sync()`.
- **Atomicity:** Writes are eager but not durable until `sync()`. Implements `Persistable`.
- **Use when:** Mapping heights/indices to fixed-size identifiers (e.g., block height → block hash). Pairs well with Journal for full record storage.
- **MCP entry:** `get_file(path="storage/src/ordinal/mod.rs", version=PINNED, start_line=0, end_line=95)`.

#### RMap (BETA)

In-memory collection of disjoint, inclusive ranges `[start, end]`. Auto-merges adjacent ranges. Used internally by Ordinal and Cache for gap tracking.

- **API:** `insert(value)`, `get(&value)`, `next_gap(value)`, `prune(max)`.
- **MCP entry:** `get_file(path="storage/src/rmap/mod.rs", version=PINNED, start_line=0, end_line=60)`.

#### Translator (BETA)

Trait for compressing keys into a smaller fixed-size representation for use in Index and Freezer. Includes `TwoCap` (2-byte) and `FourCap` (4-byte) implementations. Ships with `UintIdentity` hasher for pre-hashed keys.

- **MCP entry:** `get_file(path="storage/src/translator.rs", version=PINNED, start_line=0, end_line=25)`.

### Merkle Structures

#### MMR — Merkle Mountain Range (ALPHA)

Append-only Merkle structure for efficient inclusion proofs. Available as in-memory (`mmr::mem`) or journal-backed (`mmr::journaled`).

- **API:** Append elements, compute root digest, generate proofs for individual elements or ranges, verify proofs.
- **Use when:** Light client proofs, verifiable logs, commitment schemes. Often composed with Journal via the authenticated journal.
- **MCP entry:** `get_file(path="storage/src/mmr/mod.rs", version=PINNED, start_line=0, end_line=80)`.

#### BMT — Binary Merkle Tree (ALPHA)

Stateless binary Merkle tree built from a list of leaf digests. Supports single-element and multi-element inclusion proofs. Position-hashed leaves prevent proof malleability.

- **API:** `Builder::new(n)` → `add(digest)` → `build()` → `Tree::root()`, `proof(index)`, `proof.verify_element_inclusion(...)`.
- **Use when:** Block transaction roots, batch commitment proofs, any scenario needing a Merkle tree over a known set of items.
- **MCP entry:** `get_file(path="storage/src/bmt/mod.rs", version=PINNED, start_line=0, end_line=70)`.

### KV Traits (BETA)

`commonware-storage::kv` defines abstract traits, **not a concrete store**. These traits are implemented by higher-level primitives (QMDB, Ordinal, Freezer).

- **`Gettable`** — `get(&key) → Option<Value>`
- **`Updatable`** — `update(key, value)`, `create(key, value) → bool`, `upsert(key, update_fn)`
- **`Deletable`** — `delete(key) → bool`
- **`Batchable`** — `write_batch(iter)` for atomic multi-key operations

**Do not search for `pub struct Store`** — no such struct exists.

## Choosing a Storage Primitive

| I need to... | Use | Why |
|---|---|---|
| Track a few mutable cursors/watermarks | **Metadata** | Atomic batch commit, crash-safe, mutable |
| Append events and replay on startup | **Journal** (contiguous) | Streaming replay, crash recovery |
| Store immutable records by index + key | **Archive** | Write-once, dual lookup (index or key), prunable |
| Store immutable records by key only | **Freezer** | Minimal memory, zero write amplification |
| Cache sequential data with pruning | **Cache** | Index-only lookup, compression, gap tracking |
| Map sparse indices to fixed-size values | **Ordinal** | O(1) by position, out-of-order insert |
| Full CRUD with Merkle proofs | **QMDB** | Authenticated state, state sync (ALPHA) |
| Prove inclusion in an append-only log | **Journal (authenticated)** or **MMR** | Merkle proofs over sequential data |
| Prove inclusion in a batch of items | **BMT** | Stateless Merkle tree |

### Mutability Matrix

| Primitive | Insert | Update | Delete | Prune |
|---|---|---|---|---|
| Metadata | put | put (overwrite) | remove | retain/clear |
| Journal | append | No | No | prune (sections) |
| Archive (prunable) | put (idempotent) | No | No | prune |
| Archive (immutable) | put (idempotent) | No | No | No |
| Freezer | put | No | No | No |
| Cache | put | No | No | prune |
| Ordinal | put | put (overwrite) | No | No |
| QMDB | update/create | update | delete | prune |

## Common Composition Patterns

Individual storage primitives are building blocks — compose them for real workloads:

| Use Case | Primitives | Pattern |
|---|---|---|
| Event sourcing with mutable cursors | Journal + Metadata | Journal for events, Metadata for watermarks. Replay journal on startup to rebuild in-memory state. |
| Immutable record store | Archive (prunable or immutable) | Write-once records by index + key. Use `put_sync` for WAL semantics. |
| DKG / ceremony state | Journal + Metadata | Journal for dealings/acks, Metadata for epoch round/output/share. Prune old epochs. |
| Production blockchain | Archive + Archive | One for finalized blocks, one for certificates. Both indexed by height. |
| Idempotent cross-chain actions | Journal + Metadata | Journal for action records (append status changes), Metadata for scan watermarks. Replay on startup, deduplicate by checking journal before acting. |
| Verifiable database sync | QMDB + MMR | Authenticated state with Merkle proofs for remote sync. |
| Sequential block cache | Cache | Index-based access with gap detection for backfill. |

## Testing Patterns

Three essential test categories:

- **Basic storage test**: Init a primitive via `Primitive::init(context, config)`, write data, read it back, verify equality, call `sync()`.
- **Crash recovery test**: Write + `sync()`, drop the instance (simulates crash), re-init from the same context, verify data persists through replay or re-read.
- **Corruption test**: Write valid data, corrupt checksum bytes by overwriting blob content, verify the storage layer handles it gracefully on recovery.

All storage tests use the deterministic runtime. In deterministic mode, storage is in-memory but survives across `init()` calls with the same context/partition, simulating persistent behavior.

For Metadata-specific test patterns, see `get_file(path="storage/src/metadata/mod.rs", version=PINNED)` — it contains examples of put/get/clear, multi-sync, crash recovery with corruption, delta writes, and restart behavior.

## Key Principles

1. **Test recovery paths** — always test crash/restart behavior
2. **Corrupt data intentionally** — test truncated and corrupted payloads
3. **Test edge cases** — empty, single item, and max-size data
4. **Failures are fatal** — after a mutable method returns an error, do not continue using the instance
5. **Use conformance tests** — hash format stability tests to detect accidental changes
6. **Types must implement `Codec`** — all values stored via storage primitives need `commonware_codec::Encode + Decode` (auto-implemented as `Codec`). Use `CodecShared` for thread-safe variants, `CodecFixedShared` for fixed-size. See `get_file(path="codec/src/codec.rs", version=PINNED)` for trait definitions.
7. **`commit()` vs `sync()`** — use `sync()` for WAL patterns requiring full durability before proceeding; use `commit()` when replay-based recovery is acceptable
