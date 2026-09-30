# Utils Primitive Reference

> **Crate:** `commonware-utils`

Common utilities used across primitives.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Full API | `get_file(path="utils/src/lib.rs", version=PINNED)` |
| test_rng | `search_code(query="test_rng", crate="commonware-utils", version=PINNED)` |
| union/namespace | `search_code(query="union", crate="commonware-utils", version=PINNED)` |
| BFT calculations | `search_code(query="quorum", crate="commonware-utils", version=PINNED)` |

## Key Exports

- **`test_rng()`** / **`test_rng_seeded(N)`** — deterministic RNG for tests, critical for reproducibility. Use `test_rng()` for default seed 0, `test_rng_seeded(N)` for a custom seed. Never use `OsRng` or `StdRng::from_entropy()` in tests.
- **`union()`** / **`union_unique()`** — namespace composition for domain separation. Concatenates byte slices with length prefixes to prevent collisions.
- **`N3f1`** / **`N5f1`** — BFT fault tolerance calculations. `N3f1` computes max faults and quorum for `n = 3f + 1` (standard BFT), `N5f1` for `n = 5f + 1`. Both implement the `Quorum` trait.
- **`Array`**, **`Span`** — byte array traits for fixed-size key manipulation, used across cryptographic primitives for public keys and digests.
- **`hex()`** / **`from_hex()`** — hex encoding/decoding utilities.
