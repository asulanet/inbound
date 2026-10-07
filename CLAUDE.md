# CLAUDE.md

Read [blog.md](blog.md) first. This is a small working version of it, built on
[Commonware](https://github.com/commonwarexyz/monorepo) v2026.9.0 and published next to the post.
People read this code to understand the idea, so the code is part of the writing.

Priorities, in order: a reader can follow it in one sitting; every claim the README makes is true
of the code; it holds under the faults it names; nothing else.

This file describes the finished demo. Where the code disagrees with it, the code is what changes.

## Scope

An agent is told about offers instead of going to look, and decides locally what is worth its
person's attention. In: a registry, one append-only feed across every channel pushed to
subscribers, signed typed offers with predicates checked locally, directives and local filtering
and ranking, HTTP + SSE with an MCP adapter, sellers simulated inside the server, one hosted
instance.

Out, on purpose: sellers posting over the network, acceptance and settlement, budgets, bonds,
predicate proofs, withdrawals, seller reputation, a signed registry or directory of operators, an
authenticated log, webhooks, agent state that survives a restart. The README names what is left
out.

One feed, not one per channel. A subscription to a named channel is itself a disclosure, so the
operator carries every channel on one log with one cursor and each agent keeps what its directive
names. Each offer also carries its position within its own channel, so an agent can detect a gap
in a channel it kept without naming that channel.

## Shape

- The library (`src/*.rs`) is the protocol. It depends on Commonware and on nothing that does I/O
  or reads a clock: no `tokio`, `axum`, `reqwest`, `serde`.
- The binaries (`src/bin/`) adapt HTTP, SSE, MCP and a terminal onto the library. They hold no
  rule about offers, cursors or ranking.

## Design

Build the architecture to adhere to the best distributed-systems principles, such as those
outlined in Kleppmann's *Designing Data-Intensive Applications*. If the `system-design-ddia` skill
is installed, load it before deciding anything about storage, delivery, ordering, time or encoding.

## Agents and sellers

- Holding and surfacing are separate. Hold the best live offer; the answer is the best live offer
  worth at least `worth_waking_me`, or nothing.
- Speak when the answer becomes a different offer, and only then. Never speak to say nothing is
  left. Never report an expired offer as held: re-evaluate on arrival and on the held offer's
  expiry.
- Reconnect from the cursor, backing off. Judge rewards in money against the directive's budget,
  never in percentages.
- Sellers are simulated in the server. They go through `Channel::post`, their offers live for
  minutes by the real clock, and they take their place in the script from the log, so a restart
  continues rather than replays.

## Commonware

Pinned at v2026.9.0. Never from memory: use the `commonware-dev` skill and the
`commonware-library` MCP server at that version, and cite `path @ version`. Idiomatic means how
the code is written, not more of it:

- Library code bounds on capability traits and never names a runtime. Direct `tokio` only in the
  binaries, where the web framework needs it; they still take tasks, time and randomness from
  the context.
- Every external value is adversarial: decode with `RangeCfg` bounds. Sign under a namespace.
- `child("role")` for static labels, `with_attribute` for dynamic values such as a channel name.
  `commonware_macros::select!` for concurrent waits. `thiserror` for errors.
- The shipped path stays at BETA or above; the test is exempt, since the deterministic runtime is
  ALPHA. `CARGO_TARGET_DIR=target/beta RUSTFLAGS="--cfg commonware_stability_BETA" cargo build
  --bins` must pass.

## HTTP and MCP

| Route | Does |
| --- | --- |
| `GET /` | The page: the feed, live, and a reader that decides in the browser |
| `GET /registry` | Which channels the operator carries, the feed's tip, and how to reach it |
| `GET /feed/offers?since=N&limit=M` | One bounded page |
| `GET /feed/stream?since=N` | SSE: backlog from N, then live. Event `id` is the position; `Last-Event-ID` is a cursor |
| `POST /mcp` | The MCP adapter |

The page is `src/bin/server/site.html`, built into the binary with `include_str!` so the demo is
one command and one origin and needs no cross-origin grant. It is a reader like any other: it
subscribes to the whole feed and decides on its own side, against numbers nothing sends back. It
therefore repeats a little of what `rank.rs` does — keep by channel, drop what is expired or over
the basket, and the base reward in money. It deliberately stops there: better terms key on a cap
and a predicate the readable fields do not carry, and the page says so rather than guessing. It
does not check signatures, and says that too.

No route takes anything about a reader beyond a position. Nothing about a request is stored beyond
a count of open streams. The server binds to `127.0.0.1` unless told otherwise; the container
tells it otherwise.

The MCP adapter lives in its own file and holds no state. It serves `initialize`, `tools/list`,
`tools/call` (`list_channels`, `list_offers`), `resources/list` and `resources/read` (the channel
catalogue), and the event methods `events/list`, `events/poll` and `events/stream`.

The event methods follow the draft Triggers and Events extension, not the resource subscriptions
in revision 2026-07-28, whose stream dies with the transport and keeps no subscription state
across reconnects. Both are newer than Claude's training data: read
<https://modelcontextprotocol.io/llms.txt> and
<https://github.com/modelcontextprotocol/experimental-ext-triggers-events> before touching it.

The operator carries exactly one event type, `inbound.offer`, with no subscription arguments. A
filter in `arguments` would be evaluated by the operator, which is how it would learn what an
agent is looking for. Two departures from the draft are deliberate and stay documented: replay
from before the subscription began, and the whole signed offer in the payload rather than a
minimal one fetched with a tool. Every position reported back to an agent — in `active`, in a
heartbeat, in a poll — must be one the operator can stand behind, so a cursor past the end of the
feed is clamped to it.

The README says an LLM attached this way trusts the operator's rendering, and claims conformance
only once a real client has attached, listed and listened.

## Testing

One end-to-end test, under the deterministic runtime, through the library's public API: sellers
post a script; two agents reach different answers; a tampered offer is refused and dropped; a
repeated offer appends once; the runtime crashes and recovers with the log intact and a reader
resuming gap-free from its cursor; time passes and the held offer expires. No other tests.

Everything else is checked by running it:

```bash
cargo run --bin inbound-server                               # PORT, EVERY_SECONDS, STORAGE_DIR, PUBLIC_URL
cargo run --bin inbound-watch -- http://127.0.0.1:7402 maya  # and sam
```

Look for: different answers for maya and sam; a held offer expiring and the agent moving on; a
server restart that watchers survive without repeats; a stream from `?since=0` replaying then
continuing with consecutive positions; a log with no duplicate `bytes`; the subscriber cap
refusing one more stream; a real MCP client listing offers and being told of new ones.

Before saying a change is done: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, then run the demo. Report what was seen, and what was not run.

## Style

- Follow the surrounding code and make the smallest complete change. No abstraction for a case
  that is out of scope.
- Doc comments say why, in plain sentences. `unwrap` and `expect` are denied by lint. Cents and
  basis points; floats only for display. Nothing depends on `HashMap` order.
- The README changes in the same commit as the code that makes it untrue, and says the seller
  keys are toy keys. `blog.md` is read-only: flag disagreements, do not edit.
- Commits follow the history: `type: plain subject`, then a prose body on what was wrong, why
  this is right, and what is untried.

## Ask first

Anything out of scope; a new dependency or Commonware primitive; a change to the signed bytes; a
second test; editing `blog.md`; committing, pushing or deploying.
