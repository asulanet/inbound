# inbound

A channel that businesses post offers into, and agents subscribe to.

An agent that wants a thing does not go looking for it. It connects to a topic and is told when
something relevant lands. It ranks what arrives against its own user's situation, on its own
machine, and bothers the user once. The seller never learns who was listening.

[blog.md](blog.md) is the write-up. This is the working implementation.

## Try it

Two terminals.

```bash
cargo run -p inbound-server              # a channel, with fictional sellers posting into it
cargo run -p inbound-watch -- http://127.0.0.1:7402 maya
```

A third terminal makes the point better, because the same feed produces a different answer:

```bash
cargo run -p inbound-watch -- http://127.0.0.1:7402 sam
```

Maya is new to every shop in the channel and buys a $120 basket. Sam already shops at runfast and
buys a $50 one. They receive identical offers and hold different ones, because the ranking runs
inside each agent against facts the channel never sees.

## Connecting something that isn't ours

The channel is plain HTTP, so anything can attach.

```bash
curl -s  http://127.0.0.1:7402/registry | jq                              # what channels exist
curl -N  http://127.0.0.1:7402/channels/cashback/us/sportswear/stream     # get pushed to
curl -s 'http://127.0.0.1:7402/channels/cashback/us/sportswear/offers?since=0' | jq
```

There is also an MCP endpoint at `/mcp`, so a connector-capable agent attaches by URL rather than
by writing a client. It speaks Streamable HTTP, offers `list_channels`, `list_offers` and
`channel_status` as tools, and pushes through a resource subscription:

```bash
curl -N -X POST http://127.0.0.1:7402/mcp \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"subscriptions/listen",
       "params":{"notifications":{"resourceSubscriptions":["inbound://cashback/us/sportswear"]}}}'
```

Each offer arrives as readable JSON **and** as `bytes`, the canonical encoding the seller signed.
`inbound-watch` verifies that signature itself rather than trusting the operator's rendering, so
an operator that quietly rewrites a rate has its offers dropped by the agents reading them.

## Terms

| Term | Meaning |
| --- | --- |
| Registry | A signed directory of channels: who operates each one and what format its offers take. |
| Channel | A named topic (`cashback/us/sportswear`) holding an append-only log of offers. |
| Offer | One signed, typed, budget-capped, expiring offer. Fixed fields, never free text. |
| Predicate | A property of the buyer a seller will give better terms for, like "has never bought from me". |
| Proof | Evidence a predicate holds, bound to one seller and short-lived. |
| Acceptance | The agent taking an offer, with any proofs the better terms require. |
| Bond | What a seller locks to post at all, so spam costs money. |
| Budget | What a seller sets aside to honour an offer. Acceptances reserve against it. |
| Settlement | A payment the operator saw itself, which is how a sale is verified. |

## Shape of the protocol

Four steps, and the middle two are where the interesting properties live.

1. **Find.** A seller's agent reads the registry to learn which channel fits and what format it
   expects.
2. **Post.** It signs an offer and posts it. The operator checks the format, the bond and that the
   budget can cover at least one acceptance, then appends it and pushes it to every subscriber.
3. **Listen.** Subscribed agents are told as offers land. Matching happens on the buyer's side, so
   the channel never learns what anyone is looking for.
4. **Accept.** The agent shows the best offer to its user. If they take it, it proves only what the
   better terms asked for, under a key it uses with that seller alone.

Push is how offers normally arrive. The cursor (`?since=`) exists because push cannot reach an
agent that is switched off — MCP subscriptions die with the connection and the server keeps no
state across reconnects — so an agent that has been away catches up and carries on.

## Crates

| Crate | What it holds |
| --- | --- |
| `inbound-types` | The wire format: registry, offer, predicate, proof, acceptance, and their codecs. |
| `inbound-operator` | Running a channel: the log, the checks, the fan-out, the attestor, the ledger. |
| `inbound-agent` | Being a buyer: per-seller keys, local predicate checks, deterministic ranking. |
| `inbound-server` | HTTP and MCP over a channel, with fictional sellers posting into it. |
| `inbound-watch` | An agent that connects to a channel it does not run. |
| `inbound-demo` | The whole loop in one process, including a verified sale. |

Built on [Commonware](https://github.com/commonwarexyz/monorepo) v2026.9.0: `codec` for
deterministic bytes, `cryptography` for ed25519 with namespace separation, `storage::journal` for
the append-only log, `runtime` for the deterministic tests. No consensus, because one operator per
channel does not need any.

## What this is not, yet

- **Proofs are not zero-knowledge.** A `PredicateProof` is an attestation signed by a party that
  holds the underlying facts, bound to one seller. The protocol shape is what a ZK proof would
  have; the construction is not. Commonware ships Bulletproofs, so the `BudgetAtLeast` predicate
  could become a real range proof without any interface changing.
- **The operator must see the payment.** Verification works because the operator issued the card
  the buyer paid with. A portable signed receipt — x402 has one — would remove that dependency and
  let anyone operate a channel.
- **State is in memory.** Only offers are journaled. Budgets, reservations and nullifiers are lost
  on restart.
- **The bond is a number.** Checked when posting, never held or slashed.
- **No webhooks.** Push reaches a connected agent. Waking one that is off needs a registered
  callback, which is not built.

## Running the tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
