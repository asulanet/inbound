# Primitive Overview

> Quick-reference map of all commonware primitives, their purposes, and how to explore them via MCP.
>
> Use this file to orient yourself before diving into a specific domain. It tells you which primitive to investigate and which MCP calls will get you there.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Full library overview | `get_overview(version=PINNED)` |
| All crates with descriptions | `list_crates(version=PINNED)` |
| Specific crate docs | `get_file(path="<crate>/src/lib.rs", version=PINNED)` (see [Documentation Strategy](#documentation-strategy)) |
| Specific submodule docs | `get_file(path="<crate>/src/<submodule>/mod.rs", version=PINNED)` |

## Documentation Strategy

**Crate READMEs are minimal stubs** — they contain only the crate name, badges, and a one-line description. Do not rely on `get_crate_readme()` for understanding a crate.

The real documentation lives in Rust doc comments:

| Documentation Level | Where to Find It | MCP Call |
|---------------------|-------------------|----------|
| Crate-level overview | `<crate>/src/lib.rs` (top-level `//!` comments + public re-exports) | `get_file(path="<crate>/src/lib.rs")` |
| Submodule deep-dive | `<crate>/src/<module>/mod.rs` (detailed `//!` doc comments) | `get_file(path="<crate>/src/<module>/mod.rs")` |
| Trait/struct API | Source files with `///` doc comments | `search_code(query="pub trait <Name>", crate="commonware-<crate>")` |

**Example**: To understand Simplex consensus:
1. `get_file(path="consensus/src/lib.rs")` → crate-level traits (Automaton, Block, Relay, Reporter)
2. `get_file(path="consensus/src/simplex/mod.rs")` → 150+ lines of protocol specification, architecture diagram, design rationale

## Primitives by Domain

### Consensus & Coordination

| Primitive | Purpose | Start Here |
|-----------|---------|------------|
| **consensus** | BFT ordering (Simplex, Threshold Simplex) | `get_file(path="consensus/src/lib.rs")` → then `get_file(path="consensus/src/simplex/mod.rs")` |
| **collector** | Gather responses to committable requests | `get_file(path="collector/src/lib.rs")` |

### Networking

| Primitive | Purpose | Start Here |
|-----------|---------|------------|
| **p2p** | Authenticated peer communication | `get_file(path="p2p/src/lib.rs")` → then explore `authenticated/` or `simulated/` |
| **stream** | Encrypted message exchange | `get_file(path="stream/src/lib.rs")` |
| **broadcast** | Wide-area data dissemination | `get_file(path="broadcast/src/lib.rs")` |

### Storage & Serialization

| Primitive | Purpose | Start Here |
|-----------|---------|------------|
| **storage** | Persistent data (journal, MMR, KV, archive) | `get_file(path="storage/src/lib.rs")` → then explore submodules (`journal/`, `archive/`, `mmr/`, etc.) |
| **codec** | Deterministic serialization with size bounds | `get_file(path="codec/src/lib.rs")` |

### Data Availability

| Primitive | Purpose | Start Here |
|-----------|---------|------------|
| **coding** | Erasure coding (Reed-Solomon, ZODA) | `get_file(path="coding/src/lib.rs")` |
| **resolver** | Fetch data by fixed-length key | `get_file(path="resolver/src/lib.rs")` |

### Cryptography

| Primitive | Purpose | Start Here |
|-----------|---------|------------|
| **cryptography** | Signing, hashing, verification (Ed25519, BLS12-381, secp256r1) | `get_file(path="cryptography/src/lib.rs")` → then explore scheme submodules |
| **math** | Fields, polynomials, NTT | `get_file(path="math/src/lib.rs")` |

### Foundation

| Primitive | Purpose | Start Here |
|-----------|---------|------------|
| **runtime** | Abstract async runtime (tokio / deterministic) | `get_file(path="runtime/src/lib.rs")` |
| **parallel** | Pluggable fold parallelism strategies | `get_file(path="parallel/src/lib.rs")` |
| **utils** | Namespace composition (`union()`), test helpers | `get_file(path="utils/src/lib.rs")` |

## Stability Levels

Primitives are annotated with stability levels (transitive — primitives only depend on equal or higher stability):

| Level | Description |
|-------|-------------|
| **ALPHA** | Breaking changes expected. No migration path. |
| **BETA** | Wire and storage formats stable. Breaking changes include migration. |
| **GAMMA** | API stable. Extensively tested and fuzzed. |
| **DELTA** | Battle-tested. Bug bounty eligible. |
| **EPSILON** | Feature-frozen. Only fixes and performance. |
