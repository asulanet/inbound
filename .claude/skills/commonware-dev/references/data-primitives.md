# Data Primitives Reference

Primitives for moving and protecting data across networks.

## Overview

| Primitive | Purpose |
|-----------|---------|
| **coding** | Erasure coding for fault-tolerant data distribution |
| **stream** | Encrypted point-to-point message channels |
| **broadcast** | Wide-area message dissemination |

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Coding scheme trait | `search_code(query="pub trait Scheme", crate="commonware-coding", version=PINNED)` |
| Coding implementations | `list_files(crate="commonware-coding", version=PINNED)` then explore submodules |
| Stream API | `search_code(query="pub fn dial", crate="commonware-stream", version=PINNED)` |
| Stream config | `search_code(query="pub struct Config", crate="commonware-stream", version=PINNED)` |
| Broadcast trait | `search_code(query="pub trait Broadcaster", crate="commonware-broadcast", version=PINNED)` |
| Codec wrapping | `search_code(query="codec::wrap", crate="commonware-stream", version=PINNED)` |

---

## Erasure Coding (coding)

Distributes data so it can be reconstructed from a subset of shards.

### Scheme Trait

The `Scheme` trait defines `encode(config, data, strategy)` and `decode(config, commitment, checking_data, shards, strategy)`. Associated types include `Commitment` (attestation to shards), `Shard`, `ReShard` (for forwarding), `CheckingData`, and `CheckedShard`.

### Implementations

- **ReedSolomon** — production erasure coding. Parameterized by a `Hasher` (e.g., `ReedSolomon<Sha256>`).
- **Zoda** — Zero-knowledge One-time Data Availability. Same `Scheme` interface.
- **NoCoding** — passthrough for testing (no actual encoding).

### Configuration

`Config { minimum_shards, extra_shards }` — `minimum_shards` is the count needed for reconstruction, `extra_shards` adds redundancy (total = minimum + extra).

### Parallelism

Both `encode` and `decode` accept a `Strategy` parameter: use `Sequential` for deterministic tests, `Rayon` for production performance.

---

## Encrypted Streams (stream)

Authenticated, encrypted point-to-point message exchange using ChaCha20-Poly1305.

### Security Model

**Provided:** mutual authentication via signatures, forward secrecy (ephemeral X25519 keys), session uniqueness (handshake binding), ChaCha20-Poly1305 AEAD encryption.

**Not provided:** anonymity (identities visible in handshake), message padding (traffic analysis possible), future secrecy, 0-RTT resumption.

### API

Two entry points: `dial(context, config, peer_pk, stream, sink)` for clients and `listen(context, bouncer_fn, config, stream, sink)` for servers. Both return sender/receiver pairs for the encrypted channel. The bouncer function on `listen` decides which peers to accept.

Config includes: signing key, namespace, max message size, synchrony bound, max handshake age, handshake timeout.

Error variants: `HandshakeTimeout`, `PeerRejected`, `RecvTooLarge`, `StreamClosed`.

### Codec Wrapping

Raw byte channels from P2P can be wrapped into type-safe channels using `commonware_stream::utils::codec::wrap`. This turns `(Sender<Bytes>, Receiver<Bytes>)` into `(Sender<MyMessage>, Receiver<MyMessage>)` with automatic serialization.

---

## Broadcast (broadcast)

Reliable dissemination over wide-area networks.

### Broadcaster Trait

The `Broadcaster` trait defines `broadcast(recipients, message)` which returns a `Receiver` for collecting responses asynchronously. Associated types: `Recipients`, `Message` (must be `Codec`), `Response`.

### Buffered Implementation

The `buffered::Broadcaster` is the standard implementation. Configured with namespace, max message size, and buffer size. Created with `Broadcaster::new(context, config, sender)`.

### Integration with Consensus

A common pattern is wrapping a `Broadcaster` in a `Relay` implementation for the consensus engine. The `Relay.broadcast()` method delegates to `Broadcaster.broadcast(Recipients::All, msg)` and `Relay.send()` uses `Recipients::One(recipient)`.

---

## Best Practices

1. **Use Sequential for tests** — ensures deterministic behavior
2. **Use Rayon for production** — better throughput for encode/decode
3. **Set appropriate redundancy** — `extra_shards >= f` for f-fault tolerance
4. **Verify commitments** — always check before accepting shards
5. **Implement bouncer carefully** — prevent DoS via connection flooding
6. **Use namespaces** — for domain separation across sub-protocols
