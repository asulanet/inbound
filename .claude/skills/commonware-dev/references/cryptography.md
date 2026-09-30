# Cryptography Primitive Reference

> **Crate:** `commonware-cryptography`

Signing, verification, hashing, and cryptographic operations with multiple signature schemes.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Crate-level traits and schemes | `get_file(path="cryptography/src/lib.rs", version=PINNED)` |
| Core traits | `search_code(query="pub trait", crate="commonware-cryptography", version=PINNED)` |
| Specific scheme API | `list_files(crate="commonware-cryptography", version=PINNED)` then `get_file` into the scheme's module |
| Feature flags | `get_file(path="cryptography/Cargo.toml", version=PINNED)` |

## Core Traits

- **Signer** — holds a private key. `sign(namespace, message)` produces a signature. `public_key()` returns the associated public key.
- **Verifier** — `verify(namespace, message, signature)` checks validity.
- **Hasher** — streaming hash: `update(bytes)` feeds data, `finalize()` produces a digest. Also has a one-shot `hash(bytes)` method.

All signing/verification operations require a **namespace** for domain separation (see Namespace Convention below).

## Signature Schemes

**Ed25519** — standard signing. `PrivateKey::random(&mut rng)` for generation. Used for P2P authentication and general-purpose signatures.

**BLS12-381** — aggregatable signatures. Same `Signer`/`Verifier` API. Key feature: `Signature::aggregate(&[sig1, sig2, ...])` combines multiple signatures into one for compact certificates.

**SECP256R1 (P-256)** — ECDSA with recovery. The `recoverable` submodule supports `signature.recover_signer(namespace, message)` to derive the signer's public key from the signature.

## Hash Functions

**SHA-256** and **Blake3** both implement the `Hasher` trait. Usable in one-shot mode (`Sha256::hash(bytes)`) or streaming mode (`update` + `finalize`). CRC32 also available for non-cryptographic checksums.

## Advanced Features

- **Batch Verification** — `BatchVerifier` collects multiple (namespace, message, pk, signature) tuples and verifies them all at once, more efficient than individual checks.
- **Bloom Filter** — probabilistic set membership. `BloomFilter::new(capacity, false_positive_rate)`.
- **Transcripts** — Fiat-Shamir-style. `Transcript::new(label)`, then `append` commitments and extract `challenge` values.

## Namespace Convention

All signing operations require a namespace byte string for domain separation:

```
_COMMONWARE_<CRATE>_<OPERATION>
```

Examples: `_COMMONWARE_CONSENSUS_VOTE`, `_COMMONWARE_STREAM_HANDSHAKE`. **Changing a namespace is a breaking change.**

## Feature Flags

Each scheme is gated behind a feature flag: `ed25519`, `bls12381`, `secp256r1`, `sha256`, `blake3`, `crc32`. Check the crate's `Cargo.toml` via MCP for the current list.
