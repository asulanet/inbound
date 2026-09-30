---
name: commonware-dev
description: Expert knowledge of Commonware's distributed systems primitives — when to use each primitive, architectural patterns, and design trade-offs. Powered by the Commonware MCP server for live, version-pinned source code and documentation retrieval. Use when explaining commonware concepts, choosing between primitives for a use case, understanding consensus/P2P/storage/cryptography patterns, or learning how to compose primitives for Byzantine-fault-tolerant systems.
allowed-tools: Read, mcp__commonware-library__*
keywords:
  - commonware
  - simplex
  - bft
  - byzantine fault tolerance
  - p2p
  - bls
  - threshold signatures
  - erasure coding
  - distributed systems
---

# Commonware Development

Commonware is a Rust library of composable primitives for building distributed systems in adversarial environments. This skill uses the **Commonware MCP server** (`https://mcp.commonware.xyz`, Streamable HTTP) as its authoritative knowledge backend for all source code and documentation.

## MCP-First Resolution Protocol

**Every answer about Commonware MUST be grounded in MCP-retrieved source code or documentation.**

### Resolution Order (strict)

1. **Commonware MCP server** — authoritative for all code, APIs, types, traits, and crate docs
2. **Local reference files** (`references/`) — supplementary orientation context; may be stale
3. **Memory / training knowledge** — never authoritative; use only for high-level framing
4. **Web search / GitHub** — last resort, only for content outside the library (blog posts, discussions)

### Version Pinning

Resolve the pinned version **once per session**, then reuse it for all subsequent MCP calls:

1. Read the repo-root `Cargo.toml` → find the version in `[dependencies]`
   (e.g., `commonware-runtime = "2026.9.0"`)
2. Prefix with `v` for MCP: `"2026.9.0"` → `"v2026.9.0"`
3. Validate against `list_versions()` — call this once to confirm the version exists
4. Cache the result as your **pinned version** for the rest of the session

If the Cargo.toml version does not match any available MCP version, use the closest available version and note the discrepancy to the user. If filesystem access is unavailable (e.g., running as a subagent without Read access), call `list_versions()` and use the latest.

> **Note on `PINNED` in reference files:** Throughout the reference files, `version=PINNED` is a placeholder meaning "pass your resolved pinned version here" — it is not a literal variable or constant. Replace it with the actual version string (e.g., `version="v2026.2.0"`).

### Citation Requirement

**Every claim about Commonware behavior must cite at least one MCP-retrieved snippet.**

Format: `[file_path @ version, lines start–end]`
Example: `[consensus/src/simplex/engine.rs @ v2026.2.0, lines 42–58]`

If you cannot find MCP evidence for a claim, explicitly state it is unverified.

## MCP Tool Reference

7 tools, organized by workflow stage.

### Orientation (start here)

| Tool | Purpose |
|------|---------|
| `get_overview()` | Library README — all primitives, stability levels, structure. Call first in a session. |
| `list_crates(version)` | All crates with descriptions (primitives, examples, fuzz targets). |
| `list_versions()` | Available indexed versions. Use to validate the pinned version. |

### Discovery (find what you need)

| Tool | Purpose |
|------|---------|
| `get_crate_readme(crate, version)` | Crate README (minimal — usually just a one-liner). Use full crate name: `"commonware-consensus"`. For real documentation, prefer `get_file` on `lib.rs` or submodule `mod.rs`. |
| `list_files(crate, version)` | Recursive file tree for a crate. Explore structure before reading files. |
| `search_code(query, ...)` | Search source code across the library. Supports `mode` (`"substring"` default, `"word"` for prefix matching), `crate` (scope to one crate), `file_type` (`"rs"` default, `"md"`, `"toml"`, `"all"`), `max_results` (up to 50). |

### Retrieval (read the code)

| Tool | Purpose |
|------|---------|
| `get_file(path, version, start_line, end_line)` | Fetch file contents or a line range. Lines are 0-indexed, matching `search_code` output. **Always use `start_line`/`end_line` for large modules** (storage submodules, qmdb, index, consensus engines) — fetching without bounds can exceed output limits. Doc comments and public API are typically in the first 80–150 lines. |

### Path Convention

**`search_code` and `get_crate_readme`** use full crate names: `"commonware-consensus"`
**`get_file` and `list_files`** use directory paths: `"consensus/src/lib.rs"`

Mapping: strip `commonware-` prefix → directory name.
`commonware-consensus` → `consensus/`, `commonware-cryptography` → `cryptography/`

Exception: examples live under `examples/` (e.g., `examples/chat/src/main.rs`).

### Standard Workflows

**"How does X work?"**
1. `search_code(query="X", version=PINNED)` → locate relevant files + line numbers
2. `get_file(path, version=PINNED, start_line, end_line)` → read the implementation
3. Cite the retrieved snippet

**"Which primitive for Y?"**
1. `get_overview(version=PINNED)` → scan all primitives
2. `get_file(path="<crate>/src/lib.rs", version=PINNED)` for candidate crates
3. `search_code(query="pub trait ...", crate, version=PINNED)` → compare interfaces
4. Cite trait definitions

**"Show me the API for Z"**
1. `list_files(crate="commonware-Z", version=PINNED)` → find module structure
2. `get_file(path="Z/src/lib.rs", version=PINNED)` → read public API
3. Follow re-exports into submodules as needed

**"How does example E use primitive P?"**
1. `list_crates(version=PINNED)` → find the example crate name
2. `list_files(crate="commonware-example-E", version=PINNED)` → explore structure
3. `get_file(path="examples/E/src/main.rs", version=PINNED)` → read

### MCP Error Handling

When MCP calls return sparse or unexpected results, use these fallbacks:

| Problem | Fallback Strategy |
|---------|-------------------|
| `search_code` returns 0 results | Broaden the query: try a substring instead of exact match, drop the `crate` filter, switch to `mode="word"` for prefix matching, or try `file_type="all"` to include markdown and TOML. Increase `max_results` to 50. |
| `search_code` returns too many results | Narrow with `crate` filter or a more specific query. Use `max_results=5` to scan before committing to a deep read. |
| `get_crate_readme` returns a minimal stub | This is expected — most crate READMEs are stubs. Use `get_file(path="<crate>/src/lib.rs")` for crate-level docs and `get_file(path="<crate>/src/<module>/mod.rs")` for submodule deep-dives. |
| `get_file` returns a short file with only re-exports | The crate uses `stability_scope!` or `stability_mod!` macros to gate modules. Explore submodules via `list_files` and read the relevant `mod.rs` directly. |
| `get_file` result exceeds output limits | The file is too large (common for modules with inline tests like `index/mod.rs`, `metadata/mod.rs`, `qmdb/` files). Re-call with `start_line` and `end_line` to fetch a specific range — doc comments and public API are typically in the first 80–150 lines. Use `search_code` first to find exact line numbers if needed. |
| Version mismatch (pinned version not in MCP) | Call `list_versions()`, use the closest available version, and note the discrepancy to the user. |
| MCP server unreachable | Fall back to local reference files and `gh` CLI for the commonware GitHub repo: `gh api repos/commonwarexyz/monorepo/contents/<path> -H "Accept: application/vnd.github.raw+json"`. State that responses are unverified against the pinned version. |

## The Primitive Toolkit

For a full map of all primitives, their purposes, and MCP entry points, see `{baseDir}/references/primitive-overview.md`. Load it when you need to identify which primitives are relevant to a task.

For a quick orientation without loading the file, call `get_overview(version=PINNED)` — it lists all primitives with one-line descriptions.

## Core Principles

Stable across versions — these guide all commonware code:

- **Anti-Framework** — composable primitives, not a prescriptive framework. No runtime lock-in, no trait proliferation, no hidden complexity.
- **Runtime Agnostic** — code outside `runtime/` never imports tokio. All async operations go through runtime capability traits (`Spawner`, `Clock`, etc.).
- **Adversarial Safety** — all inputs untrusted, size limits on collections, namespace separation, signatures for authentication.
- **Simpler The Better** — prefer explicit over clever, avoid unnecessary abstraction, each function does one thing.
- **Deterministic Testing** — seeded randomness, simulated networks, Byzantine actors, crash recovery testing.

## Key Patterns

These patterns recur across commonware code. **Do not rely on the static examples below — always retrieve current implementations via MCP** using the suggested queries.

| Pattern | MCP Query |
|---------|-----------|
| Runtime trait design (`Spawner`, `Clock`, etc.) | `search_code(query="pub trait Spawner", crate="commonware-runtime")` |
| Deterministic test setup | `search_code(query="deterministic::Runner::seeded")` |
| Worker loop (`select_loop!`) | `search_code(query="select_loop", crate="commonware-macros")` |
| Application bootstrap | `get_file` on any example's `main.rs` (e.g., `examples/chat/src/main.rs`) |
| Actor-Mailbox pattern | `search_code(query="Automaton")` to find trait + implementations |
| Namespace convention | `search_code(query="_COMMONWARE_")` for namespace examples |
| Codec framing | `search_code(query="send_frame", crate="commonware-stream")` |
| Oracle + channel registration | `search_code(query="oracle.track")` for the P2P setup sequence |
| Error handling (`thiserror`) | `search_code(query="thiserror::Error")` in the relevant crate |
| Async with timeout (`select!`) | `search_code(query="select!", crate="commonware-macros")` |

### Pattern Summaries (for orientation only)

- **Runtime Isolation**: The runtime exposes capability traits (`Spawner`, `Clock`, `Storage`, `Network`, etc.) — not a single `Context` trait. Each runtime implementation (`tokio::Context`, `deterministic::Context`) is a concrete struct implementing these traits. Code outside `runtime/` bounds on these traits; never import tokio directly.
- **Deterministic Testing**: `deterministic::Runner::seeded(N)` + `test_rng()` for reproducible tests.
- **Worker Loop**: `select_loop!` macro handles graceful shutdown via `on_stopped` + message branches.
- **Application Bootstrap**: CLI args → runtime init → P2P network + oracle → channel registration → engine start.
- **Actor-Mailbox**: Mailbox implements consensus traits (`Automaton`, `Relay`, `Reporter`), forwards to Actor via mpsc channel + oneshot responses.
- **Namespace Convention**: `_COMMONWARE_<CRATE>_<OPERATION>`. Compose with `union()`. Changing a namespace is a breaking change.

## Progressive Disclosure

ONLY load what is relevant to the current task. DO NOT speculatively fetch all crates or load all reference files.

### Reference-First, Then MCP

Local reference files contain curated context — decision guides, architectural patterns, trait summaries, and targeted MCP queries. **Read the relevant reference file first** to understand the domain and identify what to look up, then use MCP to retrieve authoritative, version-pinned source code.

This two-step approach produces better results than blind MCP queries because:
- Reference files contain **MCP Quick Start tables** with the most productive queries for each topic
- They provide **decision frameworks** (e.g., "When to use Simplex vs Threshold Simplex") that MCP source code alone doesn't surface
- They explain **cross-crate patterns** (e.g., Actor-Mailbox, bootstrap sequence) that span multiple files

When reference content conflicts with MCP-retrieved code, **MCP is authoritative** — the reference may be stale.

### MCP Documentation Strategy

**Crate READMEs are minimal stubs.** `get_crate_readme()` returns only a one-line description for most crates. The real documentation is in Rust doc comments:

| Documentation Level | MCP Call |
|---------------------|----------|
| Crate-level traits and types | `get_file(path="<crate>/src/lib.rs", version=PINNED)` |
| Submodule deep-dive (protocol specs, architecture) | `get_file(path="<crate>/src/<module>/mod.rs", version=PINNED)` |
| Specific trait or struct | `search_code(query="pub trait <Name>", crate="commonware-<crate>", version=PINNED)` |

**Example**: To understand Simplex consensus, `get_crate_readme` returns 5 lines. Instead:
1. `get_file(path="consensus/src/lib.rs")` → core traits (Automaton, Block, Relay, Reporter)
2. `get_file(path="consensus/src/simplex/mod.rs")` → 150+ lines of protocol specification and architecture diagram

### Loading Guide

| Understanding... | Load These References |
|------------------|----------------------|
| Primitive overview | `{baseDir}/references/primitive-overview.md` |
| Consensus design | `{baseDir}/references/consensus.md`, `{baseDir}/references/coordination-primitives.md` |
| P2P networking | `{baseDir}/references/p2p.md`, `{baseDir}/references/data-primitives.md` |
| Storage patterns | `{baseDir}/references/storage.md`, `{baseDir}/references/codec.md` |
| Data availability | `{baseDir}/references/data-primitives.md`, `{baseDir}/references/coordination-primitives.md` |
| Cryptographic schemes | `{baseDir}/references/cryptography.md`, `{baseDir}/references/math.md` |
| Testing patterns | `{baseDir}/references/testing.md` |
| Code patterns | `{baseDir}/references/code-patterns.md` |
| Choosing primitives | `{baseDir}/references/decision-guides.md` |
| Real-world examples | `{baseDir}/references/examples.md` |

## External Projects

Tempo, Alto, and Battleware are real-world projects built on commonware. Their source is **not indexed by MCP** — use `gh` CLI for lookup.

| Project | Repo | Description |
|---------|------|-------------|
| Tempo | `tempoxyz/tempo` | EVM-compatible blockchain with dynamic validators and on-chain DKG |
| Alto | `commonwarexyz/alto` | Minimal blockchain demonstrating core commonware patterns |
| Battleware | `commonwarexyz/battleware` | Competitive game with dual consensus, encrypted moves, ELO ratings |

### Lookup Workflows

**Search code in a project:**
```
gh search code "EpochTransition" --repo tempoxyz/tempo
gh search code "threshold_simplex" --repo commonwarexyz/battleware
gh search code "Automaton" --repo commonwarexyz/alto
```

**Browse file tree (quote the URL to avoid shell glob on `?`):**
```
gh api 'repos/tempoxyz/tempo/git/trees/main?recursive=1' --jq '.tree[].path'
gh api 'repos/commonwarexyz/alto/git/trees/main?recursive=1' --jq '.tree[].path'
gh api 'repos/commonwarexyz/battleware/git/trees/main?recursive=1' --jq '.tree[].path'
```

**Read a specific file (raw content, no base64):**
```
gh api repos/commonwarexyz/alto/contents/chain/src/engine.rs -H "Accept: application/vnd.github.raw+json"
gh api repos/tempoxyz/tempo/contents/crates/commonware-node/src/epoch/manager/actor.rs -H "Accept: application/vnd.github.raw+json"
gh api repos/commonwarexyz/battleware/contents/node/src/engine.rs -H "Accept: application/vnd.github.raw+json"
```

### When to use `gh` vs MCP

- **Commonware primitives** (consensus, p2p, storage, etc.) → always use MCP
- **How Tempo/Alto/Battleware compose primitives** → use `gh` to read their source
- **Architectural patterns described in `references/examples.md`** → read the reference first, then `gh` for current source if needed

## Algorithm Deep-Dives

For theoretical understanding, blog posts are available as files in the Commonware repository:

```
search_code(query="<topic>", file_type="md", version=PINNED)
```

Or retrieve directly:

| Topic | MCP Path |
|-------|----------|
| Simplex consensus | `get_file(path="docs/blogs/threshold-simplex.html")` |
| ZODA erasure coding | `get_file(path="docs/blogs/zoda.html")` |
| Merkle Mountain Range | `get_file(path="docs/blogs/mmr.html")` |
| Secret resharing | `get_file(path="docs/blogs/reshare.html")` |
| Authenticated broadcast | `get_file(path="docs/blogs/adb-current.html")` |
