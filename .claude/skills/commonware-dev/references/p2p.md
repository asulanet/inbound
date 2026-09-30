# P2P Primitive Reference

> **Crate:** `commonware-p2p`

Authenticated peer-to-peer communication over encrypted connections using ChaCha20-Poly1305 with X25519 ECDH.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Crate-level traits and types | `get_file(path="p2p/src/lib.rs", version=PINNED)` |
| Core traits | `search_code(query="pub trait", crate="commonware-p2p", version=PINNED)` |
| Discovery network API | `search_code(query="pub struct Network", crate="commonware-p2p", version=PINNED)` |
| Simulated network API | `search_code(query="simulated", crate="commonware-p2p", version=PINNED)` |
| Link configuration | `search_code(query="pub struct Link", crate="commonware-p2p", version=PINNED)` |
| Example setup | `get_file(path="examples/chat/src/main.rs", version=PINNED)` |

## Core Traits

- **Sender** — `send(recipients, message, priority)` dispatches messages. Returns the list of failed recipients. Extends `LimitedSender` for rate-limited sending.
- **Receiver** — `recv()` yields `(PublicKey, IoBuf)` — the sender's identity and raw message bytes.
- **Blocker** — `block(peer)` disconnects and blocks a misbehaving peer.

## Key Types

- **Recipients** — enum with three variants: `All` (broadcast), `Some(Vec<P>)` (multicast), `One(P)` (unicast).
- **Channel** — a `u64` identifier. Different message types get different channels, each with independent rate limiting and backlog.

## Authenticated Network (Production)

The `discovery::Network` is the production P2P layer. Configure with namespace, signing key, max message size, synchrony bound, and handshake parameters. Create with `Network::new(context, config)` which returns `(network, oracle)`.

Setup sequence:
1. `oracle.track(epoch, authorized_keys)` — register authorized peers
2. `network.register(channel_id, quota, backlog)` — create channels (must be done before start)
3. `network.start()` — begins accepting connections

## Simulated Network (Testing)

The `simulated::Network` provides controlled network simulation for deterministic tests. Create with `Network::new(context, config)`, then configure links between peers using `oracle.add_link(pk1, pk2, link)` where `Link` has `latency`, `jitter`, and `success_rate` fields.

Supports dynamic modification: `oracle.update_link()` changes link properties, `oracle.remove_link()` creates partitions.

## Security Model

**Provided:** mutual authentication via signatures, forward secrecy (ephemeral X25519 keys), ChaCha20-Poly1305 encryption.

**Not provided:** anonymity, message padding (traffic analysis possible), future secrecy.

## Best Practices

1. **Use labeled contexts** — `context.with_label("network")` for debugging
2. **Register all channels before starting** — channels cannot be added after `network.start()`
3. **Handle send failures** — check the returned failed recipients list
4. **Test network partitions** — use `oracle.remove_link()` to simulate splits
5. **Use the priority flag** for time-critical messages (votes, certificates)
