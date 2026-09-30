# Commonware Code Patterns

> Patterns and principles that guide how commonware code is written.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Application bootstrap example | `get_file(path="examples/chat/src/main.rs", version=PINNED)` |
| Actor-Mailbox implementations | `search_code(query="Automaton", version=PINNED)` |
| select_loop! macro | `search_code(query="select_loop", crate="commonware-macros", version=PINNED)` |
| select! macro | `search_code(query="select!", crate="commonware-macros", version=PINNED)` |
| Namespace usage | `search_code(query="_COMMONWARE_", version=PINNED)` |
| Codec wrapping | `search_code(query="codec::wrap", crate="commonware-stream", version=PINNED)` |
| Channel registration | `search_code(query="network.register", version=PINNED)` |

## Design Principles

- **The Simpler The Better** — code should look obviously correct. Prefer explicit over clever. Avoid unnecessary abstraction. Each function does one thing. Comments explain *why*, code explains *what*.
- **Test Everything** — deterministic, comprehensive testing. Seeded randomness for reproducibility. Simulated networks for controlled chaos. Byzantine actors for adversarial testing. Crash recovery for durability.
- **Performance Sensitive** — prefer `Bytes` over `Vec<u8>` for zero-copy. Use static dispatch over dynamic where possible. Batch operations when beneficial. Profile before optimizing.
- **Adversarial Safety** — all inputs are untrusted. Size limits enforced on all collections. Namespace separation prevents cross-protocol attacks. Signatures required for authentication.
- **Abstract Runtime** — code outside `runtime/` never imports tokio. Use `context.sleep()`, `context.spawn()`, etc. Enables deterministic testing.

## Naming Conventions

| Element | Convention | Examples |
|---------|------------|----------|
| Types | `PascalCase` | `PrivateKey`, `BlockHeader` |
| Functions | `snake_case` | `verify_signature`, `get_block` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_MESSAGE_SIZE`, `DEFAULT_TIMEOUT` |
| Traits | Action-oriented or `-able` | `Signer`, `Verifier`, `Viewable` |

## Namespace Conventions

All cryptographic operations use namespaces for domain separation: `_COMMONWARE_<CRATE>_<OPERATION>`. Examples: `_COMMONWARE_CONSENSUS_VOTE`, `_COMMONWARE_STREAM_HANDSHAKE`, `_COMMONWARE_CRYPTOGRAPHY_BLS12381_DKG`.

Compose sub-protocol namespaces within an application using `union()` from `commonware-utils`: define a root namespace, then derive sub-namespaces for P2P, consensus, indexer, etc.

**Changing a namespace is a breaking change.**

## Application Composition Patterns

### Application Bootstrap Sequence

Every consensus application follows this canonical wiring order:
1. Parse CLI args → derive Ed25519 identity from seed
2. Initialize runtime (`tokio::Runner` with storage directory)
3. Create `discovery::Network` + `Oracle`
4. `oracle.track(epoch, authorized_keys)` to register authorized peers
5. Register channels with rate limits + backlog per message type (must be before `network.start()`)
6. Initialize application actor + consensus engine
7. `network.start()` + `engine.start(channels...)`

Fetch a complete example: `get_file(path="examples/chat/src/main.rs", version=PINNED)`

### Actor-Mailbox Pattern

The standard way to decouple application logic from the consensus engine:

- **Mailbox** — a thin adapter that implements consensus traits (`Automaton`, `Relay`, `Reporter`). Receives calls from the engine and forwards them to the actor via an mpsc channel, returning responses via oneshot channels.
- **Actor** — runs in its own task, receives `Message` variants via `mpsc::Receiver`. Does the real work: building proposals, verifying blocks, updating state.

This keeps all application logic in the Actor, which is easy to test independently, while the Mailbox is a mechanical forwarding layer.

### Codec Wrapping for Typed Channels

Raw byte P2P channels can be wrapped into type-safe message channels using `commonware_stream::utils::codec::wrap`. This converts raw `(Sender, Receiver)` pairs into typed `(Sender<MyMessage>, Receiver<MyMessage>)` with automatic `Codec` serialization/deserialization. Pass a `RangeCfg` for size-constrained types.

### Oracle Track + Channel Registration

The universal P2P setup sequence: create network, track authorized peers via `oracle.track(epoch, keys)`, register channels (each gets a channel ID, rate limit `Quota`, and backlog size), then start the network. Channels must be registered before `start()`. Each channel gets independent rate limiting.

## Async Patterns

- **Worker loop**: `select_loop!` macro for long-running tasks. Handles graceful shutdown via `on_stopped` and message processing in other branches.
- **Timeout**: `select!` macro racing an operation against `context.sleep(duration)`.
- **CPU-intensive work**: `context.shared(true).spawn()` routes to the blocking pool.
- **Async traits**: Use RPITIT (return-position `impl Future` in traits) — `fn method(&self) -> impl Future<Output = T> + Send`.

## Code Review Focus

When reviewing commonware code, prioritize:
1. **Correctness** — does it work as intended?
2. **Performance** — is it efficient for the use case?
3. **Adversarial safety** — what happens with malicious input?
4. **Runtime isolation** — is it runtime-agnostic?
5. **Determinism** — can it be tested reproducibly?
