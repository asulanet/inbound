# Inbound: trustless incoming communication for agents

Draft

## Introduction

Instinct's [Trusted Person network](https://x.com/noahrshinn/status/2097794967574028448) is one of the first examples of letting your agent talk to other people's agents and settle the back-and-forth of getting people together.  However, your agent speaks only to agents belonging to the people closest to you. Keeping the trusted set of agents closed has a few benefits: it prevents your agent from being prompt-injected or manipulated, it also prevents token costs from blowing up due to persistent communication between a large number of agents.

However, untrusted communication between agents or between your agent and people is (in our opinion) a much more interesting problem. How do we allow our agent to be safely spoken to by someone it has every reason to distrust?

In this post, we outline a simple implementation of a system we're calling "Inbound" that will allow agents to receive incoming messages if/when it wants to get pinged.

## Web search

Today, an agent uses web search to read information from sources it doesn't know or trust. When the user asks for a used car under $50,000, the agent goes out, reads pages with card sales offers, and comes back with a view. This is already untrusted incoming communication.

However, a search is a snapshot of the internet at one moment, and the person buying might be okay waiting 3 weeks before the pull the trigger on the car.  Offers may show up, expire or get updated during this period.

The workaround for a snapshot is to keep taking them. Poll every hour, stack up snapshots, diff them. However, we know from traditional software engineering that polling is expensive and tokens are expensive.

The third problem is that the web in its current form was designed for humans and somewhat repurposed for agents. Worse, a page can even manipulate the agent reading it. [Reword a product description](https://arxiv.org/abs/2508.02630) and agents pick it more often; plant the right text and they can be hijacked outright.

None of this is an argument against search. It is the right tool for a question. It is the wrong tool for a standing interest.

## Ads and the first link

For the longest time, Google has intermediated all internet traffic. When a user wants to get somewhere on the internet: they query google, google gives them a result and then they get to where they want to go.

This puts Google in a position of extreme power. Google monetizes this power through ads. Humans read the Google results page top down and stop as soon as they reach an acceptable result.

A good agent should not have this early stopping problem. It should read every relevant listing before coming to a conclusion on "where to go" or "what to buy". NB: agents today can still be fooled in many ways, our qualification of "good" allows us to extrapolate to what seems like a inevitable future.

We've set all of the first three sections to convey: untrusted targeted communication is valuable, ads on web search is a form of untrusted target communication but with agents becoming the primary users of the internet, the questions becomes: can we do better than ads or can we make it so that more of the value from ads is forwarded to consumers.

## Inbound

Inbound is a way for a business to put an offer in front of someone's agent without going through a search engine.  We'll talk about this in the context of the user "buying" something via an agent and the agent becoming the intermediary between the user and the business they'd like to buy from.

Technically, Inbound is a protocol that maintains a set of **topics** run by operators. Agents can subscribe to topics to receive updates about them. The agent can then go and rank the **offers** in a given topic and present a subset to its user.  **Sellers** can then post to different channels, they don't need to know who is subscribing to the channel or how their results will be ranked. NB: sellers may need to be whitelisted or have to post a bond to be able to prevent sybil but we won't dive into this topic in the post to keep things higher level.

Why would anyone want this?

For the user:  Their agent states an interest and offers keep arriving against it, it compares all of them and instead of taking a one time snapshot the user's agent is able to make a more informed decision about "what to buy".  The agent can always do the web search so that inbound offers strictly improve on naive web search results.  This format also allows the user to be "targeted" without leaking their information.

For the business: Businesses don't have to pay anything to the topic operator until they sell something. Moreover, they know that subscribers of the topic are likely to be genuinely interested in the topic.

NB: the operator can get paid either when a sale happens via inbound. Or there is a version of this setup where the offers have masked information about the seller, so when the agent picks a group of sellers for its user, the agent might have to pay to learn who the seller actually is (this moves the cost of discovery from the seller to the user which might be unintended but its just a potential design choice of letting people maintain their privacy).

Below, we'll go through some detail about Inbound and then finish off with sharing some other ways Inbound can be used in interesting ways.

## Channels

A channel is a named topic, something like `cars/ev/us`. Sellers post into it, agents subscribe to it.  Sellers post a bond so in case of any griefing the seller can be penalized.

This idea of broadcasting your intent to buy something isn't new. Crypto already works this way: you publish the trade you want and solvers compete to fill it.  The consumer version has also been tried in a few different formats before but:

- **Sellers refused to compete in public.** TrueCar, a US car-buying site, had dealers bid against each other on price for each buyer. About a third of its 5,600 dealers quit within three months in early 2012. TrueCar then dropped the intent product.
- **The request became a lead.** Ask a comparison site for a mortgage and it doesn't go and find you a mortgage. It sells your details to five lenders, and all five call you.

Ideally, we can design a protocol to circumvent some of these shortcomings understanding that the new consumers of the internet are meaningfully different from the existing consumers of the internet. Offers go into a topic, not to one named buyer, so a seller is publishing a standing offer rather than being dragged into a bidding war over one person. And who the buyer is never exposed publicly by the agent, so there's nothing to sell as a lead.

As previously stated, search is a snapshot. Inbound channels provide an elegant method for persistent and continuous discovery that is also token efficient. It does ask more of sellers, who now have to post offers into the topics their business fits. We think that's a fair trade. Every business once had to build a website so search engines could find it. Posting to Inbound is the agent-era version: a business runs a small offer-posting agent that helps keep their inventory and the posted offers in sync.

## Operators

Operators create channels. Buyers subscribe to them.

An operator lets subscribed agents in and pushes updates to them when new offers come in. It also confirms when a sale happens.

An operator's first job is making sure a channel isn't empty when an agent subscribes. There are two ways to fill one:

1. **Search once, sell many times.** The operator runs a continuous web search for its topic and posts what it finds into the channel. This effectively amortizes the cost of the web search about a topic across several agents meaning that if there is sufficient demand for the topic, the operator is profitable.
2. **Build seller distribution.** The operator signs up sellers to its topic, the sellers post their own offers into the channel.  Fresh offers keep arriving without the operator having to find them.

## Protocol

The Inbound protocol itself can be small. It defines three objects, so a seller's agent knows how to write an offer and a buyer's agent knows how to read one:

| Object | What it is | Who signs it |
| --- | --- | --- |
| Registry | A public directory of channels. Agents use it to find which channels exist and who operates each channel. | Registry Maintainer |
| Channel | A named topic (`cars/ev/us`) holding an append-only log of offers. It also defines the format of the offers in the channel. | Channel Operator |
| Offer | One typed, budget-capped, expiring offer posted into a channel. | Seller |

Offers stick to the typed format their channel defines, with no free text. Free text would open up the system to prompt injection-like attacks that would be dangerous when broadcasted to agents at scale.

Here's how one offer moves through it:

1. **Find the channel:** A seller's agent looks up the right channel in the registry, along with the offer format that channel expects.
2. **Post:** It signs an offer in that format and posts it. The channel's operator checks it fits the format and appends it to the channel's log. NB: here we can also check if the signed offer has a bond associated with it to prevent griefing.
3. **Listen in on the channel:** Buyer agents subscribed to the channel read new offers from where they last left off, and match them against what their user wants. That matching happens on the buyer's side, so the channel never learns what anyone is looking for.
4. **Accept the offer:** When an offer fits, the agent shows it to the user. If they take it, the agent redeems it with the seller and the operator confirms the sale happened.

## Example

Say a running-shoe brand called Runfast wants more orders. It sets aside $5,000 and posts one offer into `cashback/us/sportswear`:

```
seller:     runfast
reward:     10% back, up to $15
min spend:  $60
window:     October
budget:     $5,000
signed:     runfast
```

Agents subscribed to that channel pull it down. One of them figures it's worth showing its user, next to everything else that landed in the channel that month. Runfast never finds out who saw it.

The user buys $84 of shoes with the card in their wallet. The operator issued that card, so it sees the purchase go through: runfast, $84, October. Thirty days later, once the returns window has closed, the user gets $8.40 back and runfast gets a bill for $8.40 plus a flat fee.

Runfast paid for a sale, not for a click.

## Extension: private claims

The protocol above works without knowing anything about the buyer. A natural next step is letting sellers make better offers to buyers with specific properties. For example, a verified budget or being a first-time customer.

The cleanest way is to let sellers specify when they're willing to improve their offers. Alongside an offer, a seller posts **predicates** it would give better terms for, like "budget ≥ $30k" or "has never bought from runfast". The buyer's agent checks them locally and, if its user meets one, proves it with a zero-knowledge proof when accepting.

The proof shows the predicate is true without revealing anything behind it: the seller learns the budget is over $30k, not what it is.

## Demo

*[insert demo here]*
