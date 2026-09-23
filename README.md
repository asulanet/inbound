# inbound

A channel businesses post offers into, and agents subscribe to.

An agent that wants something does not go looking for it. It connects to a topic and is told when
an offer lands. It ranks what arrives against its own user's situation, on its own machine, and
bothers the user once. The seller never learns who was listening.

[blog.md](blog.md) is the write-up. This is a small working version of it.

## Try it

```bash
cargo run --bin inbound-server
cargo run --bin inbound-watch -- http://127.0.0.1:7402 maya
```

A third terminal makes the point, because the same operator gives a different answer:

```bash
cargo run --bin inbound-watch -- http://127.0.0.1:7402 sam
```

Ten shops post into three channels. Maya asked her agent to watch shoes and would spend $120, so
it subscribes to one channel. Sam asked for coffee and shoes and would spend $50, so his
subscribes to two. Runfast never wins for Sam: its better terms are for someone who has never
bought there, and he has.

## How it works

```mermaid
sequenceDiagram
    autonumber
    participant S as sellers
    participant O as channel
    participant M as maya's agent
    participant A as sam's agent

    M->>O: subscribe
    A->>O: subscribe
    Note over O: two queues, and nothing else.<br/>not who they are, not what they want.

    S->>O: signed offer
    O->>O: check signature, append to log
    O--)M: push
    O--)A: push

    Note over M: verify from the signed bytes<br/>check predicates against own data<br/>shuffle, then rank
    Note over A: same offer, different data

    M-->>M: better → runfast $28.18
    A-->>A: nothing better, pass
```

Steps 6 and 7 are the whole idea: the channel writes to the agents, and neither agent asked. Steps
8 and 9 are why there is no leaderboard — the ranking runs on each agent's own machine, against
facts that never left it, so the same offer is worth different amounts to different people.

Who ends up knowing what:

| | Sees the offers | Sees who is listening | Sees what anyone wants | Sees the ranking |
| --- | --- | --- | --- | --- |
| Seller | its own | no | no | no |
| Channel | all | a count | no | no |
| Agent | all | itself | its own user | its own |

The channel's advantage over a search engine is not that it knows more. It knows strictly less. It
is just pointed the other way.

## How an agent finds a channel

One URL in, everything needed to subscribe out.

```bash
curl -s http://127.0.0.1:7402/registry
```

```json
{ "channels": [
    { "channel": "cashback/us/sportswear",
      "offers": 12,
      "listening": 2,
      "stream": "http://127.0.0.1:7402/channels/cashback/us/sportswear/stream",
      "since":  "http://127.0.0.1:7402/channels/cashback/us/sportswear/offers?since=0" } ],
  "mcp": "http://127.0.0.1:7402/mcp" }
```

An agent reads that, keeps the channels its directive names, catches up through `since`, and
holds `stream` open. There is no registration and no key exchange: reading a channel is
anonymous, and the operator learns only that its subscriber count went up.

A directory listing several operators would be one more JSON file above this. It does not exist
yet, and for a demo pointing an agent at one URL is enough.

## Directives

An agent is a person's facts plus their standing instruction:

```rust
Directive {
    about: "running shoes before the marathon",
    channels: ["cashback/us/sportswear"],
    budget: 12_000,          // what they would actually spend, in cents
    worth_waking_me: 2_500,  // do not interrupt below this
}
```

The directive decides three things: which channels to subscribe to, what the reward is judged
against (so a $4 coffee and a $30,000 car are not compared on percentages), and when the person
wanted to be interrupted.

Holding and surfacing are separate decisions on purpose. Sam's agent held `stride` at $7.56 and
said nothing for five offers, because his bar was $8. When a later offer crossed it, the agent
spoke. An agent that conflates the two either nags or sits quietly on something good.

## Connecting something else

Nothing about the two agents above is special. The channel is plain HTTP:

```bash
curl -N  http://127.0.0.1:7402/channels/cashback/us/sportswear/stream          # be told
curl -s 'http://127.0.0.1:7402/channels/cashback/us/sportswear/offers?since=0' # catch up
```

There is an MCP endpoint at `/mcp` too, so a connector-capable agent attaches by URL instead of
someone writing a client. It has `list_offers` as a tool, and pushes through a resource
subscription:

```bash
curl -N -X POST http://127.0.0.1:7402/mcp \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{}}'
```

Every offer arrives as readable JSON **and** as `bytes`, the canonical encoding the seller signed.
`inbound-watch` verifies that signature itself, so an operator that quietly rewrites a rate on the
way past has its offers dropped by the agents reading them.

## What's here

| File | What it holds |
| --- | --- |
| `src/offer.rs` | The offer: signed, typed fields, no free text. Better terms unlocked by a predicate. |
| `src/channel.rs` | The channel: an append-only log, and the fan-out to everyone listening. |
| `src/rank.rs` | The buyer's side: local predicate checks, then verify, shuffle, sort. |
| `src/directive.rs` | What a person asked their agent to watch for, and what it does about an offer. |
| `src/sellers.rs` | Ten made-up shops across three channels, so the channels have something in them. |
| `src/bin/server.rs` | The registry, HTTP, SSE and MCP over every channel. |
| `src/bin/watch.rs` | An agent that reads the registry, subscribes, and acts on its directive. |

Offers carry fixed fields and nothing a seller could write prose into, which is the point: an
agent reading this channel cannot be talked round the way a web page talks it round. Ranking is
deterministic code over those fields, with the offers shuffled first, so neither arrival order nor
anything the seller chooses can move the result.

Built on [Commonware](https://github.com/commonwarexyz/monorepo) v2026.9.0 — `codec` for the
deterministic bytes a signature needs to mean anything, `cryptography` for namespaced ed25519,
`storage::journal` for the log, `runtime` underneath. No consensus: one operator per channel.

## What this deliberately leaves out

The blog describes a protocol. This is the part of it that makes the demo work, and nothing else.

- **No acceptance, settlement or ledger.** Offers are received and ranked; taking one is out of
  scope here, so nothing is ever bought and no seller is ever billed.
- **One operator.** The registry lists that operator's own channels. A directory of operators
  would sit above it and does not exist.
- **Predicates are checked, never proved.** The agent decides locally whether better terms apply.
  Proving that to a seller, in zero knowledge or otherwise, is the extension the blog describes.
- **No bond.** Anyone with a keypair can post. Making spam cost something needs a deposit the
  operator actually holds.
- **Push only reaches a connected agent.** The cursor exists because an agent that was switched
  off missed everything; waking one that is off needs a webhook, which is not built.
