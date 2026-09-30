# Runtime Primitive Reference

> **Crate:** `commonware-runtime`

The abstract async runtime that enables runtime-agnostic code and deterministic testing.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Context trait definition | `search_code(query="pub trait Context", crate="commonware-runtime", version=PINNED)` |
| Deterministic runner | `search_code(query="deterministic::Runner", crate="commonware-runtime", version=PINNED)` |
| Tokio runtime | `search_code(query="tokio::Executor", crate="commonware-runtime", version=PINNED)` |
| Error types | `search_code(query="pub enum Error", crate="commonware-runtime", version=PINNED)` |
| Storage operations | `search_code(query="fn open", crate="commonware-runtime", version=PINNED)` |
| Full module structure | `list_files(crate="commonware-runtime", version=PINNED)` |

## The Core Principle

**All code outside `runtime/` must use `Context` for async operations, never direct runtime imports.**

This enables:
- Deterministic testing with controlled time
- Reproducible failures with seeded execution
- Crash recovery testing
- Network simulation

## When to Use Each Runtime

| Runtime | When |
|---------|------|
| **Deterministic** | Tests, debugging, simulating network conditions, crash recovery testing |
| **Tokio** | Production, real network I/O, real time |

## Context Capabilities

The `Context` trait provides all async operations:

- **Task spawning** — `context.spawn(closure)` for basic tasks, `context.with_label("name").spawn(closure)` for labeled tasks (aids debugging), `context.shared(true).spawn(closure)` for CPU-intensive work that should use the blocking pool (signature verification, hashing, encoding).
- **Time** — `context.sleep(duration)` advances time (deterministic runtime only advances on sleep), `context.now()` returns current timestamp.
- **Storage** — `context.open(partition, key)` returns a `(blob, size)` tuple. Blob supports `read_at`, `write_at`, `sync`. In deterministic mode, storage is in-memory but survives `context.recover()`.
- **Network** — `context.bind(addr)` for listeners, `context.dial(addr)` for connections. In deterministic mode, use `commonware_p2p::simulated::Network` instead.
- **Metrics** — `context.counter(name, value)`, `context.gauge(name, value)`, `context.histogram(name, value)`.

## Deterministic Runtime

Created with `Runner::seeded(seed)` for a fixed seed, `Runner::timed(duration)` to add a timeout, or `Runner::new(config)` for full configuration. Start with `executor.start(|context| async { ... })`.

### Why Determinism Matters

| Problem | Solution |
|---------|----------|
| Flaky tests | Same seed = same execution |
| Hard to debug | Reproducible failures |
| Time-dependent bugs | Controlled time advancement |
| Network race conditions | Simulated, ordered delivery |

### Crash Recovery Testing

Test with a loop: run the application, simulate crash, create a new runner via `Runner::from(prev_context)`, call `context.recover()` to restore state, verify correctness. Storage data persists across `recover()` calls.

## Error Types

`BindFailed(addr)`, `ConnectionFailed(addr)`, `WriteFailed`, `ReadFailed`, `Exited`, `Timeout`. Fetch current definitions via MCP.

## Critical Rules

1. **Never import tokio directly** outside of `runtime/` crate
2. **Use `futures` crate** for async primitives (channels, streams)
3. **Always label spawned tasks** in tests for debugging
4. **Use deterministic runtime** for all tests
5. **Use `select!` for timeouts** — not tokio's timeout

## Common Patterns

- **Worker with graceful shutdown**: `select_loop!` macro with `on_stopped` branch for cleanup + message receive branches.
- **Operation with timeout**: `select!` macro racing the operation against `context.sleep(timeout)`.
- **Labeled sub-components**: Create separate labeled contexts (`context.with_label("consensus")`, `context.with_label("network")`) for each subsystem.

To fetch implementations of these patterns: `search_code(query="select_loop!", version=PINNED)` or look at any example's `main.rs`.
