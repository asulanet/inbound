# Commonware Testing Guide

Commonware uses deterministic testing to ensure reproducibility. This guide covers testing patterns and conformance testing.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Deterministic runner | `search_code(query="deterministic::Runner", crate="commonware-runtime", version=PINNED)` |
| test_rng usage | `search_code(query="test_rng", crate="commonware-utils", version=PINNED)` |
| Simulated network | `search_code(query="simulated", crate="commonware-p2p", version=PINNED)` |
| Byzantine testing | `search_code(query="conflicter", crate="commonware-consensus", version=PINNED)` |
| Crash recovery | `search_code(query="start_and_recover", crate="commonware-runtime", version=PINNED)` |
| Conformance macro | `search_code(query="conformance_tests", crate="commonware-conformance", version=PINNED)` |
| Example test files | `search_code(query="#[test]", crate="commonware-consensus", max_results=5, version=PINNED)` |

## Deterministic Runtime

**Critical:** Use the deterministic runtime for all tests. Create a `deterministic::Runner` with `Runner::seeded(seed)` for a fixed seed, `Runner::timed(duration)` for timeout, or `Runner::new(config)` for full configuration (seed + timeout). Start with `executor.start(|context| async move { ... })`.

### Configuration

Three construction modes: `Runner::seeded(42)` for seed-only, `Runner::timed(Duration::from_secs(30))` for timeout-only, and `Runner::new(Config::new().with_seed(42).with_timeout(...))` for full control.

## Deterministic RNG

Use `test_rng()` (default seed 0) or `test_rng_seeded(N)` (custom seed) from `commonware_utils`. Never use `OsRng` or `StdRng::from_entropy()` in tests — these break reproducibility.

## Simulated Network

Create a simulated network with `Network::new(context, Config { max_size })`, which returns a `(network, oracle)` pair. Call `network.start()`, then use the oracle to configure links between peers: `oracle.add_link(pk1, pk2, Link { latency, jitter, success_rate })`. Remove links for partition testing with `oracle.remove_link(pk1, pk2)`.

The broad query `search_code(query="simulated", crate="commonware-p2p", version=PINNED)` returns the full implementation across mod.rs, network.rs, and ingress.rs.

## Byzantine Testing

Replace honest nodes with Byzantine actors to test fault tolerance. The `mocks::conflicter::Conflicter` sends conflicting messages. After running, verify Byzantine nodes are blocked via `oracle.blocked()`. Fetch the Conflicter implementation: `search_code(query="conflicter", crate="commonware-consensus", version=PINNED)`.

## Crash Recovery Testing

Test with a loop: run the application under a deterministic runner, simulate a crash, create a new runner via `Runner::from(prev_context)`, and verify state recovery. The `start_and_recover` pattern is the canonical entry point. Storage data persists across `context.recover()` calls.

## Conformance Testing

Protect against unintended format changes by hashing encoded outputs.

### Adding Conformance Tests

**Step 1: Implement Arbitrary** — add `impl arbitrary::Arbitrary<'_> for MyType` behind `#[cfg(feature = "arbitrary")]`, delegating each field to `u.arbitrary()?`.

**Step 2: Add Test Module** — create a `#[cfg(test)] mod conformance` that uses `commonware_conformance::conformance_tests!` with `CodecConformance<MyType>`.

**Step 3: Run Tests** — use `just test-conformance` (all crates), `just test-conformance -p commonware-codec` (specific crate), or `just regenerate-conformance` (update hashes after a breaking change).

### TOML Format

Conformance files use TOML with sections like `["MyType"]` containing `n_cases = 65536` and `hash = "abc123def456..."`.

## Codec Round-Trip Testing

Test that all custom types survive encode then decode. Always test round-trip for each enum variant, invalid/corrupt data handling (e.g., corrupted discriminant bytes), and edge cases (empty, max size). Fetch codec patterns: `search_code(query="fn test_", crate="commonware-codec", version=PINNED)`.

## Realistic Network Simulation

Use the simulated network with real-world parameters for protocol estimation.

Key capabilities:
- **Bandwidth limiting** per peer (egress and ingress independently)
- **Directional links** (A→B can differ from B→A)
- **Dynamic updates** during simulation (`oracle.update_link()`)
- **Network partitions** via `oracle.remove_link()`

Configure per-peer bandwidth with `oracle.limit_bandwidth(identity, egress_cap, ingress_cap)`. Use real latency data for links (e.g., 67ms for us-east-1 → eu-west-1) and simulate lossy networks by setting `success_rate` below 1.0.

## Plan-Based Test Orchestration

For complex distributed system tests, use structured test plans. A `Plan` struct captures: `seed` (deterministic execution), `total` (total participants), `per_round` (rotating committee sizes), `link` (network conditions), `crash` (crash simulation config), and `failures` (forced epoch failures).

This pattern enables:
- **Parameterized test suites** — same logic, different conditions
- **Rotating committees** — verify behavior across validator set changes
- **Forced failures** — test specific epoch failure scenarios
- **Deterministic replay** — same seed reproduces exact execution

## Crash Simulation Patterns

Beyond basic crash recovery, test realistic failure modes:

- **Random crashes** — participant crashes every N epochs, stays offline for M epochs, then restarts with state recovery from persistent storage. Validates journal replay and cache rebuilding.
- **Delayed starts** — participant joins after the system has been running for N epochs. Validates state sync and catch-up protocols.
- **Full shutdown** — all participants restart simultaneously. Validates that consensus resumes from last persisted view.

Test that after any crash pattern: state is recovered correctly from journals/metadata, idempotent operations (e.g., ack generation) produce the same results, and pruned data does not cause errors on replay.

## Best Practices

1. **Always use deterministic runtime**
2. **Label all spawned tasks** — `context.with_label("name")`
3. **Use test_rng()** for all randomness
4. **Test progressive degradation** — start ideal, add failures
5. **Test Byzantine actors** — replace honest with conflicters
6. **Test crash recovery** — save and restore context
7. **Test network partitions** — split-brain scenarios
8. **Verify determinism** — same seed = same result
9. **Add conformance tests** — for all serialized types
10. **Test codec round-trips** — for all custom wire types
11. **Simulate realistic networks** — use real latency data when available
12. **Structure complex tests with plans** — parameterize conditions, not logic
