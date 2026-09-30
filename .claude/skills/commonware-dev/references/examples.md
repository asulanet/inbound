# Real-World Commonware Examples

> Projects built with commonware that demonstrate how to compose primitives effectively.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| List all examples | `list_crates(version=PINNED)` (filter for commonware-bridge, commonware-chat, etc.) |
| Bridge example | `list_files(crate="commonware-bridge", version=PINNED)` |
| Chat example | `list_files(crate="commonware-chat", version=PINNED)` |
| Reshare (DKG) example | `list_files(crate="commonware-reshare", version=PINNED)` |
| Estimator example | `list_files(crate="commonware-estimator", version=PINNED)` |
| Any example main.rs | `get_file(path="examples/<name>/src/main.rs", version=PINNED)` |

> **Note:** Tempo, Alto, and Battleware are external projects built on commonware. Their source is not available via MCP — use `gh` CLI for live source lookup. The descriptions below capture their architectural patterns for reference.
>
> | Project | Repo |
> |---------|------|
> | Tempo | `tempoxyz/tempo` |
> | Alto | `commonwarexyz/alto` |
> | Battleware | `commonwarexyz/battleware` |
>
> The MCP server indexes these example crates: bridge, chat, estimator, flood, log, reshare, sync.

## Tempo

**What it is**: An EVM-compatible blockchain with dynamic validator sets and on-chain DKG.

**Commonware primitives used**:
- `commonware-consensus` (Simplex) for BFT consensus
- `commonware-cryptography` (BLS12-381) for threshold signatures and DKG
- `commonware-p2p` (lookup network) for authenticated peer discovery
- `commonware-storage` (immutable archives) for finalized blocks/certificates
- `commonware-broadcast` (buffered) for message dissemination
- `commonware-runtime` for async coordination

**Key architectural patterns**:

1. **Multi-epoch consensus with multiplexing**: Tempo runs multiple Simplex engines simultaneously (one per active epoch) using channel muxers. The `EpochManager` actor coordinates epoch transitions — spinning up new engines with fresh key shares on enter, gracefully shutting down on exit.

2. **On-chain DKG integration**: The `DKGManager` actor monitors finalized blocks for DKG outcomes from smart contracts, then coordinates threshold key generation. It reads validator sets from the on-chain DKG contract, participates in BLS12-381 DKG ceremonies when selected, and signals epoch transitions to the EpochManager.

3. **Actor model with mailbox coordination**: Components communicate via typed mailboxes and implement the Reporter pattern for finalization events. Reporters are composed hierarchically — for example, nesting epoch manager, executor, and DKG manager mailboxes together.

4. **Application automaton for execution layer**: The `Application` actor implements `commonware_consensus::Automaton` with `handle_genesis()` for epoch boundary blocks, `handle_propose()` calling reth for new payloads, and `handle_verify()` validating blocks via EVM execution.

5. **Storage architecture**: Uses two immutable archives with freezer tables — `finalizations_by_height` for certificates indexed by height and `finalized_blocks` for block data with zstd compression.

6. **7 P2P channels with rate limiting**:
   | Channel | Purpose | Rate Limit |
   |---------|---------|------------|
   | VOTES | Vote messages | 128/sec |
   | CERTIFICATES | Cert exchange | 128/sec |
   | RESOLVER | Block fetching | 128/sec |
   | BROADCASTER | Consensus msgs | 8/sec |
   | MARSHAL | State sync | 8/sec |
   | DKG | Key generation | 128/sec |
   | SUBBLOCKS | Subblock data | 128/sec |

**When to reference Tempo**: Building systems with validator set changes, on-chain governance, EVM integration, or multi-epoch consensus.

---

## Alto

**What it is**: A minimal "wicked fast" blockchain that demonstrates core commonware patterns without execution complexity.

**Commonware primitives used**:
- `commonware-consensus` (Simplex + Marshal) for BFT ordering
- `commonware-cryptography` (BLS12-381, Ed25519) for signatures
- `commonware-storage` (immutable archives) for block persistence
- `commonware-broadcast` (buffered) for block buffering
- `commonware-p2p` (authenticated) for networking
- `commonware-resolver` for block fetching

**Key architectural patterns**:

1. **Simple block structure**: Blocks contain only consensus-critical data — parent digest (SHA256 of parent), height (sequential), timestamp (milliseconds since epoch), and a pre-computed digest.

2. **Fixed epoch design**: Uses `FixedEpocher` with `EPOCH_LENGTH = u64::MAX` — no validator set changes, simplifying the consensus flow.

3. **Marshaled application wrapper**: The `Marshal` actor wraps the application automaton, handling block storage in immutable archives, finalization certificate persistence, and block resolution from peers.

4. **Application interface implementation**: Genesis is hardcoded from a known seed string. Verification checks timestamp synchrony bounds (500ms tolerance against the current context time).

5. **5 P2P channels for consensus**:
   | Channel | Purpose | Rate Limit |
   |---------|---------|------------|
   | PENDING | New proposals | 128/sec |
   | RECOVERED | Block sync | 128/sec |
   | RESOLVER | Block fetching | 128/sec |
   | BROADCASTER | Consensus | 8/sec |
   | MARSHAL | State machine | 8/sec |

6. **Client API (REST + WebSocket)**:
   - `POST /seed` — Submit proposals
   - `GET /block/{height|digest|latest}` — Query blocks
   - `WS /consensus/ws` — Real-time consensus stream

**Data flow**: Proposal → Notarization (1/3+ sigs) → Finalization (2/3+ sigs) → Archive

**When to reference Alto**: Building simple consensus systems, learning commonware patterns, or when you don't need transaction execution.

---

## Battleware

**What it is**: A competitive game system with turn-based battles, ELO ratings, and cryptographic fairness.

**Commonware primitives used**:
- `commonware-consensus` (threshold_simplex + aggregation) for dual consensus
- `commonware-cryptography` (BLS12-381, Ed25519, TLE) for signatures and encryption
- `commonware-storage` (freezer, ordinal, ADB, MMR) for state management
- `commonware-p2p` with 8 independent broadcast networks
- `commonware-resolver` for peer coordination

**Key architectural patterns**:

1. **Dual consensus system**: Uses `threshold_simplex::Engine` for view-based block consensus and `aggregation::Engine` for epoch-based certificate aggregation. Two supervisors coordinate — `ViewSupervisor` manages view subscriptions and leader election, `EpochSupervisor` monitors epoch changes for aggregation.

2. **Complex application state via Adb**: Accounts with nonces, battle lobbies (max 128 players, 25 view expiry), active battles with creature state, and leaderboards with ELO ratings.

3. **Mempool with backpressure**: 32,768 max total transactions, 16 max per account, priority ordered by nonce.

4. **Seeder actor for randomness**: Produces and distributes random seeds using `Ordinal` storage for persistence, with backfill protocol for sync and per-peer rate limits.

5. **Aggregator actor for certificates**: Prunable cache for recent certificates, persistent ordinal for history, generates MMR proofs for state verification.

6. **Encrypted moves (anti-front-running)**: Players submit encrypted move selections (`Move(Ciphertext)`) then reveal and finalize with a proof (`Settle(Signature)`).

7. **8 P2P networks**:
   | Network | Purpose |
   |---------|---------|
   | pending | New transactions |
   | recovered | Block recovery |
   | resolver | Peer discovery |
   | broadcast | Consensus messages |
   | backfill | Historical sync |
   | seeder | Random seeds |
   | aggregator | Certificate collection |
   | aggregation | Aggregation consensus |

8. **Game mechanics**: Creatures generated from deterministic hash (300 skill points), 4 move types + no-op with usage limits proportional to power, ELO calculation with K=32 and tanh-based probability.

**When to reference Battleware**: Building applications with complex state machines, game logic, encrypted inputs, MMR proofs, or multi-network architectures.
