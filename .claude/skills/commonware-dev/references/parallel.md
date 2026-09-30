# Parallel Primitive Reference

> **Crate:** `commonware-parallel`

Pluggable parallel execution strategies that enable deterministic testing while maintaining production performance.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Strategy trait | `search_code(query="pub trait Strategy", crate="commonware-parallel", version=PINNED)` |
| Sequential impl | `search_code(query="Sequential", crate="commonware-parallel", version=PINNED)` |
| Rayon impl | `search_code(query="Rayon", crate="commonware-parallel", version=PINNED)` |
| Usage in erasure coding | `search_code(query="Strategy", crate="commonware-coding", version=PINNED)` |
| Module structure | `list_files(crate="commonware-parallel", version=PINNED)` |

## When to Use This Primitive

### Use commonware-parallel when:
- Writing code that benefits from parallel execution
- Need to maintain determinism in tests
- Using erasure coding (`RS::encode`, `RS::decode`)
- Processing batches of cryptographic operations
- Any CPU-bound operation that can be parallelized

### Why pluggable parallelism?
The key insight is that **determinism and parallelism are at odds**: parallel execution introduces non-deterministic ordering, tests require reproducibility for debugging, and production requires performance. Commonware-parallel solves this by abstracting the execution strategy.

## Strategies

### Sequential

Executes operations one at a time, in order.

**Use Sequential when**: running tests (determinism required), debugging (easier to trace), or small workloads (parallelism overhead not worth it).

Usage: pass `Sequential` as the strategy parameter, e.g., `RS::encode(&config, data, Sequential)`. Same input always produces same output order.

### Rayon

Parallel execution using the Rayon work-stealing scheduler.

**Use Rayon when**: production workloads, benchmarking realistic performance, or CPU-bound operations with sufficient parallelism.

Usage: pass `Rayon::default()` as the strategy parameter, e.g., `RS::encode(&config, data, Rayon::default())`. Utilizes all available cores.

## Pattern: Strategy-Parameterized Functions

Design functions to accept a generic `P: Strategy` parameter. Call `parallel.map(items, |item| process(item))` inside. In tests, pass `Sequential` for determinism; in production, pass `Rayon::default()` for performance. This lets the same logic work in both contexts without conditional compilation.

## Decision Guide

| Situation | Strategy | Reason |
|-----------|----------|--------|
| Unit tests | `Sequential` | Reproducible failures |
| Integration tests | `Sequential` | Consistent behavior |
| Benchmarks | `Rayon` | Realistic performance |
| Production | `Rayon` | Maximum throughput |
| Debugging prod issues | `Sequential` | Isolate race conditions |
| Small batches (<100 items) | Either | Parallelism overhead may not help |
| Large batches (1000+ items) | `Rayon` | Significant speedup |

## Implementation Notes

### The Strategy Trait

The `Strategy` trait requires `Clone + Send + Sync + 'static` and defines a single method `map<T, U, F>(&self, items: &[T], f: F) -> Vec<U>` where input items must be `Sync` (readable from multiple threads), output must be `Send` (movable to collecting thread), and the closure must be `Sync` (callable from multiple threads). Fetch the full definition: `search_code(query="pub trait Strategy", crate="commonware-parallel", version=PINNED)`.

### Common Use in Erasure Coding

The `RS::encode` and `RS::decode` functions in `commonware-coding` accept a `Strategy` parameter. Pass `Rayon::default()` for production encoding/decoding and `Sequential` for tests. See the erasure coding integration: `search_code(query="Strategy", crate="commonware-coding", version=PINNED)`.

### Thread Pool Considerations

Rayon uses a global thread pool by default: sized to number of CPU cores, work-stealing for load balancing, shared across all Rayon operations. For isolated parallelism, consider custom thread pools in specific use cases.

## Anti-Patterns

**Don't mix strategies in a single test.** Using `Rayon` and `Sequential` in the same test and comparing results will fail due to ordering differences. Pick one strategy per test.

**Don't use Rayon for I/O-bound work.** Rayon is designed for CPU-bound parallelism. For async I/O, use the async runtime (`futures::future::join_all`) instead — Rayon would block threads and waste cores.

**Don't parallelize tiny operations.** For very small collections (e.g., 3 elements), the parallelism overhead exceeds the benefit. Just use a regular iterator loop.
