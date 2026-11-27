# Inbound: untrusted incoming communication for agents

Draft

## Introduction

Instinct's [Trusted Person network](https://x.com/noahrshinn/status/2097794967574028448) is an early example of letting your agent talk to other people's agents and settle the back-and-forth of getting people together. However, your agent speaks only to agents belonging to the people closest to you. Keeping the trusted set of agents closed has a few benefits: it prevents your agent from being prompt-injected or manipulated, and it keeps token costs from blowing up due to persistent communication between a large number of agents.

However, untrusted communication between agents or between your agent and people is (in our opinion) a much more interesting problem. How do we allow our agent to be safely spoken to by someone it has every reason to distrust?

A closed network handles untrusted communication by blocking it. Inbound lets it in through a fixed format, so the trust boundary shifts from *who is speaking* to *what can be said*. It's the move that tamed SQL injection: a parameterized query doesn't ask who typed into the form; it leaves no place where data can become a command.

In this post, we outline a simple implementation of a system we're calling "Inbound" that lets an agent receive incoming messages when it wants them.

## Web search

Today, an agent uses web search to read information from sources it doesn't know or trust. When the user asks for a used car under $50,000, the agent goes out, reads pages with car sales offers, and comes back with a view. This is already untrusted incoming communication.

However, a search is a snapshot of the internet at one moment, and the person buying might be okay waiting three weeks before they pull the trigger on the car. Offers may show up, expire or get updated during this period.

The workaround for a snapshot is to keep taking them. Poll every hour, stack up snapshots, diff them. Hourly for three weeks is 504 searches, each a fresh read of mostly the same pages.

Then there's the web itself, which in its current form was designed for humans and somewhat repurposed for agents. Worse, a page can even manipulate the agent reading it. [Reword a product description](https://arxiv.org/abs/2508.02630) and agents pick it more often; [plant the right text](https://arxiv.org/abs/2510.25779) and, in a simulated marketplace, they can be hijacked outright.

None of this is an argument against search. It is the right tool for a question. It is the wrong tool for a standing interest.

## Ads and the first link

For the longest time, Google has been the front door to the internet. When a user wants to get somewhere, they query Google, Google gives them a result, and then they get to where they want to go.

This puts Google in a position of extreme power. Google monetizes this power through ads. Humans tend to read the Google results page top down and stop as soon as they reach an acceptable result.

It's tempting to say a good agent wouldn't stop early, but today's agents have a version of the same problem: the study linked above finds shopping agents favor certain positions on the page, and which positions changes between model versions. Making the model read every listing isn't the fix either, since that costs the same tokens and time as polling. The fix is to stop making the model do the reading. If offers arrive as small typed records, ordinary code can compare all of them and the model sees only the few that survive. Position stops being for sale when the thing doing the ranking doesn't look at position.

So untrusted, targeted communication is valuable, and ads on web search are one form of it. With agents becoming the primary users of the internet, the question becomes: can we do better than ads, or at least pass more of their value on to consumers?

## Inbound

Inbound is a way for a business to put an offer in front of someone's agent without going through a search engine.  We'll talk about this in the context of the user "buying" something via an agent and the agent becoming the intermediary between the user and the business they'd like to buy from.

Technically, Inbound is a protocol that maintains a set of **topics** run by operators. Agents subscribe to an operator to receive updates from all of its topics. The agent can then go and rank the **offers** in the topics its user cares about and present a subset to its user.  **Sellers** can then post to different channels, they don't need to know who is listening or how their results will be ranked. NB: sellers may need to be whitelisted or have to post a bond to be able to prevent sybil but we won't dive into this topic in the post to keep things higher level.

Why would anyone want this?

For the user:  Their agent holds a standing interest and offers keep arriving against it, it compares all of them and instead of taking a one time snapshot the user's agent is able to make a more informed decision about "what to buy".  The agent can always do the web search so that inbound offers strictly improve on naive web search results.  Nothing about the user travels to find these offers. The offers travel to the agent, every agent gets the same ones, and the choosing happens where the user's facts already are.

For the business: Businesses don't have to pay anything to the topic operator until they sell something. Moreover, they know an offer is only shown to a person whose agent judged it worth their attention.

NB: the operator gets paid when a sale happens via inbound.
>ben: we should define what constitutes a sale and how can we tell if a sale was generated by inbound

Below, we'll go through some detail about Inbound and then finish off with sharing some other ways Inbound can be used in interesting ways.

## Channels

A channel is a named topic, something like `cars/ev/us`. Sellers post into it, agents read from it.  Sellers post a bond so in case of any griefing the seller can be penalized.

This idea of broadcasting your intent to buy something isn't new. Crypto already works this way: you publish the trade you want and solvers compete to fill it. The consumer version has also been tried in a few different formats before, but:

- **Sellers refused to compete in public.** TrueCar, a US car-buying site, had dealers bid against each other on price for each buyer. Between December 2011 and February 2012 its franchise-dealer count [fell from about 5,600 to 3,600](https://www.sec.gov/Archives/edgar/data/1327318/000104746914004966/a2220192zs-1a.htm), and TrueCar ended the reverse auction.
- **The request became a lead.** Ask a comparison site for a mortgage and it doesn't go and find you a mortgage. It passes your details to up to five lenders, and they call you.

Ideally, we can design a protocol to circumvent some of these shortcomings understanding that the new consumers of the internet are meaningfully different from the existing consumers of the internet. Offers go into a topic, not to one named buyer, so a seller is publishing a standing offer rather than being dragged into a bidding war over one person. And the buyer never sends a request, so there is no request to pass on as a lead. A subscription can be a lead too, though: someone watching `cars/ev/us` for three weeks is in the market, name or no name. So an agent subscribes to everything the operator carries and keeps the topics it cares about. Sellers post into narrow topics; buyers never say which ones they read.

This doesn't stop sellers watching each other, though. A channel is public, so a seller can read its rivals' offers and answer them. Where prices are already public, as with a shop's cashback rate, that changes nothing. Where the margin lives in a price nobody publishes, as with cars, expect sellers to post a list price or stay out. That's why our example below is cashback, not a car.

As previously stated, search is a snapshot. Inbound channels provide an elegant method for persistent and continuous discovery that is also token efficient. It does ask more of sellers, who now have to post offers into the topics their business fits. We think that's a fair trade. Every business once had to build a website so search engines could find it. Posting to Inbound is the agent-era version: a business runs a small offer-posting agent that helps keep their inventory and the posted offers in sync.

## Operators

Operators create channels. Buyers subscribe to an operator and get everything it carries.

An operator lets subscribed agents in and pushes updates to them when new offers come in. It also confirms when a sale happens.

An operator's first job is making sure a channel isn't empty when an agent subscribes. There are two ways to fill one:

1. **Search once, sell many times.** The operator runs a continuous web search for its topic and posts what it finds into the channel. This effectively amortizes the cost of the web search about a topic across several agents meaning that if there is sufficient demand for the topic, the operator is profitable.
>ben: not sure if I follow this one - how does an offer found through continuous web search become a valid channel entry? don't we need the seller signature and bond, or is this some parallel path?

2. **Build seller distribution.** The operator signs up sellers to its topic, the sellers post their own offers into the channel.  Fresh offers keep arriving without the operator having to find them.

## Protocol

The Inbound protocol itself can be small. It defines three objects, so a seller's agent knows how to write an offer and a buyer's agent knows how to read one:

| Object | What it is | Who signs it |
| --- | --- | --- |
| Registry | A public directory of channels. Agents use it to find which channels exist and who operates each channel. | Registry Maintainer |
| Channel | A named topic (`cars/ev/us`) holding an append-only log of offers. It also defines the format of the offers in the channel. | Channel Operator |
| Offer | One typed, budget-capped, expiring offer posted into a channel. | Seller |

>ben: not sure about the append-only log of offers. what if offer changes or sells out? im leaning towards some kind of offer lifecycle that could be confirmed at any time.
>
>ben: without getting into implementation details too much, there's also some latency games with claiming an offer. e.g.:
>
> t0: agent fetches the latest offers, filters them and presents them to the user
>
> t1: user comes back, reviews the presented offers, find the one they want for a coffee nearby
>
> t2: user goes to the cafe and tries to claim the offer but at some point between t0 and t2 the offer expired or is otherwise void
>
> seems like we want some kind of handshake-claim of an offer to give the user guarantees about it 

Offers stick to the typed format their channel defines, with no free text, not even the seller's name: an offer names its seller by key. Free text would open up the system to prompt-injection-style attacks that would be dangerous when broadcast to agents at scale. Offers carry no links either. A link an agent follows tells the seller that someone looked, and when.

Here's how one offer moves through it:

1. **Find the channel:** A seller's agent looks up the right channel in the registry, along with the offer format that channel expects.
2. **Post:** It signs an offer in that format and posts it. The channel's operator checks it fits the format and appends it to the channel's log. NB: here we can also check if the signed offer has a bond associated with it to prevent griefing.
3. **Listen broadly:** Buyer agents subscribe to everything the operator carries. They read new offers from where they last left off, keep the topics their user cares about, and match those offers against what their user wants. The filtering and the matching both happen on the buyer's side, so the operator learns that an agent is listening and nothing about what for.
4. **Accept the offer:** Receiving an offer needs nobody's approval; accepting one spends money, so when an offer fits, the agent shows it to the user. If they take it, the agent redeems it with the seller and the operator confirms the sale happened.

## Example

Say a running-shoe brand called Runfast wants more orders. It sets aside $5,000 and posts one offer into `cashback/us/sportswear`:

```
seller:     <runfast's key>
reward:     10% back, up to $15
min spend:  $60
window:     October
budget:     $5,000
signature:  <over all of the above>
```

Every agent listening to that operator pulls it down. One of them figures it's worth showing its user, next to everything else that landed in the channel that month. Runfast never finds out who saw it.

The user buys $84 of shoes with the card in their wallet. The operator issued that card, so it sees the purchase go through: runfast, $84, October. It would have seen that purchase anyway. What it does not learn is whether an agent had anything to do with it, or what else that agent weighed. There was no click, so Runfast's site saw an ordinary customer. Thirty days later, once the returns window has closed, the user gets $8.40 back and runfast gets a bill for $8.40 plus a flat fee.

Runfast paid for a sale, not for a click.

## Extension: private claims

The protocol above works without knowing anything about the buyer. A natural next step is letting sellers make better offers to buyers with specific properties. For example, a verified budget or being a first-time customer.

The cleanest way is to let sellers specify when they're willing to improve their offers. Alongside an offer, a seller posts **predicates** it would give better terms for, like "budget ≥ $30k" or "has never bought from runfast". The buyer's agent checks them locally and, if its user meets one, proves it with a zero-knowledge proof when accepting.

The proof shows the predicate is true without revealing anything behind it: the seller learns the budget is over $30k, not what it is.

## Privacy

Inbound aims to keep the buyer's interests, budget, and shortlist inside their agent during discovery. Agents receive the same broad feed and filter it without reporting which offers they read. Reading an offer should trigger no seller requests or viewing reports.

The agent provider remains trusted with the user's context. [Tracking Conversations](https://arxiv.org/pdf/2604.27438) and [Prompt like a Butterfly](https://jorgegarciaherrero.com/wp-content/interactivos/20260916-Prompt-like-a-butterfly-sting-like-a-tracker-%28clean%29.pdf) document conversation content and metadata reaching third parties through chatbot interfaces. The provider's interface, logs, and analytics therefore also need to protect the user's preferences and offer comparisons.

With broad subscriptions, this is what each party can directly see:

| Party | Who the user is | Which topics their agent reads | Which offers it weighed | What the user bought |
| --- | --- | --- | --- | --- |
| Seller | At purchase, like any shop | No | No | Its own sale |
| Operator | A network address | No | No | No, unless it settles the sale |
| Card issuer | Yes | No | No | Yes, as with any purchase |
| Agent provider | Yes | Yes | Yes | If shared with the agent |

The "No" entries mean no direct reporting; inference is still possible. Choosing a car-only operator may reveal an interest in cars. If one party fills multiple roles, its knowledge combines, as with the operator and card issuer in the example.

A reservation could reveal the chosen offer before purchase, so any reservation handshake needs its own privacy design. Eligibility proofs reveal the condition proved, potentially about an identifiable customer, and repeated thresholds can reveal more. An agent should prove only the predicate its chosen offer needs, at the point it is required.

## Demo

*[insert demo here]*
