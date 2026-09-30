# Coordination Primitives Reference

Primitives for coordinating distributed operations.

## Overview

| Primitive | Purpose |
|-----------|---------|
| **collector** | Gather responses to committable requests (votes, acks) |
| **resolver** | Fetch data by key/hash from the network |

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Collector traits | `search_code(query="pub trait", crate="commonware-collector", version=PINNED)` |
| Resolver traits | `search_code(query="pub trait", crate="commonware-resolver", version=PINNED)` |
| Collector module structure | `list_files(crate="commonware-collector", version=PINNED)` |
| Resolver config | `search_code(query="pub struct Config", crate="commonware-resolver", version=PINNED)` |
| Usage in examples | `search_code(query="resolver", file_type="rs", version=PINNED, max_results=5)` |

---

## Response Collection (collector)

Collects responses to committable requests — used for gathering votes, acknowledgements, or other aggregatable responses from validators.

### Core Traits

- **Originator** — sends requests and tracks responses. `send(recipients, request)` dispatches to validators, returns the list of successful sends. `cancel(commitment)` aborts a pending collection.
- **Handler** — receives requests and produces replies. `process(origin, request, response_sender)` validates and responds.
- **Monitor** — notified when responses accumulate. `collected(handler, response, count)` fires as each response arrives, enabling threshold logic (e.g., create certificate when `count >= threshold`).

### Usage Patterns

**Consensus vote collection**: An `Originator` sends a vote request to all validators. Each `Handler` validates and replies with a signed vote. The `Monitor` tracks responses and creates a certificate once a threshold is reached.

**Generic request/response**: The same pattern works for any aggregation — acknowledgements, attestations, data requests. The `Committable + Digestible + Codec` bounds on requests/responses ensure they can be identified, hashed, and serialized.

---

## Data Resolution (resolver)

Fetches data identified by fixed-length keys when you have a hash but not the data itself.

### Core Traits

- **Consumer** — notified when data arrives. `deliver(key, value)` returns a bool (accept/reject — use this to validate that the hash matches). `failed(key, failure)` handles fetch failures.
- **Resolver** — initiates fetches. `fetch(key)` asks the network, `fetch_targeted(key, targets)` asks specific peers (e.g., the block author), `cancel(key)` and `clear()` manage pending requests.

### Usage Patterns

**Block resolution**: Check local cache first. If missing, call `resolver.fetch(hash)`. The `Consumer.deliver()` callback validates the hash, stores the block, and notifies waiters.

**Targeted fetch**: When you know who has the data (e.g., the block proposer), use `fetch_targeted(key, vec![author])` for faster resolution.

The P2P resolver implementation (`commonware_resolver::p2p::Resolver`) is configured with namespace, max value size, timeout, and max retries.

---

## Best Practices

1. **Always validate delivered data** — check that the hash matches the key in `deliver()`
2. **Use targeted fetch** when you know the likely source
3. **Track response counts** for threshold-based decisions
4. **Cancel stale requests** to free resources
5. **Handle Byzantine responses** — reject invalid data silently
6. **Set appropriate timeouts** — network conditions vary
