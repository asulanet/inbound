# Consensus Primitive Reference

> **Crate:** `commonware-consensus`

Byzantine-fault-tolerant ordering using the Simplex consensus protocol.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Crate-level traits | `get_file(path="consensus/src/lib.rs", version=PINNED)` |
| Simplex protocol spec | `get_file(path="consensus/src/simplex/mod.rs", version=PINNED)` |
| All public traits | `search_code(query="pub trait", crate="commonware-consensus", version=PINNED)` |
| Simplex engine API | `search_code(query="pub struct Engine", crate="commonware-consensus", version=PINNED)` |
| Aggregation API | `search_code(query="Aggregator", crate="commonware-consensus", version=PINNED)` |
| Config struct | `search_code(query="pub struct Config", crate="commonware-consensus", version=PINNED)` |
| Example usage | `get_file(path="examples/chat/src/main.rs", version=PINNED)` |

## Core Traits

**Block traits** — `Epochable`, `Heightable`, `Viewable` provide round coordinates. `Block` combines these with `Codec + Digestible + Committable` and adds a `parent()` method for chain linking.

**Automaton** — the state machine interface your application implements:
- `genesis(epoch)` → returns the genesis digest for an epoch
- `propose(context)` → returns a receiver that yields a proposed digest
- `verify(context, payload)` → returns a receiver that yields a validity bool

**Relay** — the communication interface for broadcasting/sending consensus messages.

**Reporter** — receives event notifications (commits, faults, etc.) from the engine.

## Core Types

`Epoch` (u64), `Height` (u64), `View` (u32) are type aliases for round coordinates. `Round` bundles all three. `Context` carries round info plus parent commitment and timestamp — passed to `Automaton` methods.

## Simplex Protocol

The main consensus engine. Configured with a namespace, epoch number, weighted validator set, propose/vote timeouts, and the local validator's identity. Created via `Engine::new(context, config, automaton, relay, reporter)` and started with `engine.start(pending_messages, recovered_state, resolver)`.

Threshold Simplex is a variant using BLS threshold signatures for smaller certificates.

## Aggregation

`Aggregator` collects BLS signatures toward a threshold. Call `add(voter_pk, signature)` for each vote, check `has_threshold()`, then `finalize()` to produce a certificate.

## Testing Patterns

- **Basic consensus test**: Use `deterministic::Runner::seeded(N)`, create N validators (3f+1), set up a simulated `Network`, start engines, and wait for commit progress.
- **Byzantine test**: Replace some validators with the `mocks::conflicter::Conflicter` to test equivocation. Check `oracle.blocked()` for detected misbehavior.
- **Supervisor verification**: Use `mocks::supervisor::Supervisor` to subscribe to commit events and assert no faults were recorded.

To fetch test examples: `search_code(query="#[test]", crate="commonware-consensus", version=PINNED)`

## Best Practices

1. **Use deterministic runtime** for all tests
2. **Test with f Byzantine nodes** to verify safety
3. **Test network partitions** to verify liveness recovery
4. **Verify determinism** — same seed must produce same execution
5. **Use supervisors** for distributed property assertions
6. **Label all actors** with `context.with_label()` for debugging
