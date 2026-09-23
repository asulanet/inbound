//! What a person told their agent to watch for.
//!
//! A directive is the standing instruction: the topics to listen on, what the person would
//! actually spend, and how good something has to be before it is worth interrupting them. It is
//! the agent's whole brief, and none of it is ever sent to a channel or a seller.

use crate::{Buyer, Offer, Ranked, rank};

#[derive(Clone, Debug)]
pub struct Directive {
    /// What the person would say they are after, in their own words. For display only; no part of
    /// the protocol reads it.
    pub about: String,
    /// The topics to subscribe to.
    pub channels: Vec<String>,
    /// What they would actually spend, in cents. Rewards are judged against this, so a $4 coffee
    /// and a $30,000 car are not compared on percentages.
    pub budget: u64,
    /// Do not interrupt for less than this, in cents.
    pub worth_waking_me: u64,
    /// Stop watching after this.
    pub until: u64,
}

/// What the agent decided to do about an offer that just arrived.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Below the bar the person set, or not better than what is already held.
    Ignore,
    /// Worth holding, but not worth a notification on its own.
    Hold { seller: String, worth: u64 },
    /// Good enough that the person asked to hear about it.
    Surface { seller: String, worth: u64 },
}

/// One person's agent: the facts it knows about them, and the brief they gave it.
pub struct Agent {
    pub buyer: Buyer,
    pub directive: Directive,
    best: Option<u64>,
    surfaced: bool,
    pub ignored: u32,
}

impl Agent {
    pub fn new(buyer: Buyer, directive: Directive) -> Self {
        Self {
            buyer,
            directive,
            best: None,
            surfaced: false,
            ignored: 0,
        }
    }

    pub fn best_so_far(&self) -> Option<u64> {
        self.best
    }

    /// Rank everything held and decide whether this person wanted to hear about it.
    ///
    /// Two separate bars, because they answer different questions. "Is this the best I have seen"
    /// decides what to hold. "Did they ask to be woken for this" decides whether to speak. An
    /// agent that conflates them either nags or stays quiet about something good.
    pub fn consider(&mut self, seen: &[Offer], now: u64) -> Decision {
        let budget = self.directive.budget;
        let mut buyer = self.buyer.clone();
        buyer.basket = budget;

        let Some(top) = top_for(seen, &buyer, now) else {
            self.ignored += 1;
            return Decision::Ignore;
        };
        if !self.best.is_none_or(|best| top.worth > best) {
            self.ignored += 1;
            return Decision::Ignore;
        }
        self.best = Some(top.worth);

        let seller = crate::name_of(&top.offer.body.seller);
        if top.worth >= self.directive.worth_waking_me && !self.surfaced {
            self.surfaced = true;
            return Decision::Surface {
                seller,
                worth: top.worth,
            };
        }
        Decision::Hold {
            seller,
            worth: top.worth,
        }
    }
}

fn top_for(seen: &[Offer], buyer: &Buyer, now: u64) -> Option<Ranked> {
    rank(seen, buyer, now).into_iter().next()
}

/// The two people in the demo. Same channels are available to both; they ask for different things
/// and know different things, so they end up somewhere different.
pub fn directive_for(who: &str) -> Directive {
    match who {
        "sam" => Directive {
            about: "coffee, and shoes if something is unusually good".into(),
            channels: vec!["cashback/us/coffee".into(), "cashback/us/sportswear".into()],
            budget: 5_000,
            worth_waking_me: 800,
            until: u64::MAX,
        },
        _ => Directive {
            about: "running shoes before the marathon".into(),
            channels: vec!["cashback/us/sportswear".into()],
            budget: 12_000,
            worth_waking_me: 2_500,
            until: u64::MAX,
        },
    }
}
