use crate::{Offer, Predicate};
use commonware_cryptography::ed25519::PublicKey;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use std::collections::HashMap;

/// One person's situation, as their own agent knows it. None of this is ever sent anywhere.
#[derive(Clone, Debug)]
pub struct Buyer {
    pub name: String,
    pub basket: u64,
    pub balance: u64,
    pub spent_with: HashMap<PublicKey, u64>,
}

impl Buyer {
    /// Whether a predicate is true of this buyer, checked locally.
    pub fn holds(&self, predicate: &Predicate) -> bool {
        match predicate {
            Predicate::NeverBoughtFrom(seller) => {
                self.spent_with.get(seller).copied().unwrap_or(0) == 0
            }
            Predicate::BudgetAtLeast(amount) => self.balance >= *amount,
        }
    }

    fn qualifies(&self, offer: &Offer) -> Vec<Predicate> {
        offer
            .body
            .better
            .iter()
            .map(|terms| terms.predicate.clone())
            .filter(|predicate| self.holds(predicate))
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Ranked {
    pub offer: Offer,
    pub worth: u64,
}

/// Verify, drop what does not apply, shuffle, then sort on the numbers. Position in the feed and
/// anything a seller might write cannot influence the result, because neither is an input.
pub fn rank(offers: &[Offer], buyer: &Buyer, now: u64) -> Vec<Ranked> {
    let mut live: Vec<&Offer> = offers
        .iter()
        .filter(|offer| offer.verify())
        .filter(|offer| offer.live_at(now))
        .filter(|offer| buyer.basket >= offer.body.min_spend)
        .collect();

    live.shuffle(&mut ChaCha20Rng::seed_from_u64(now));

    let mut ranked: Vec<Ranked> = live
        .into_iter()
        .map(|offer| Ranked {
            worth: offer
                .body
                .reward_for(&buyer.qualifies(offer))
                .on(buyer.basket),
            offer: offer.clone(),
        })
        .collect();

    ranked.sort_by(|left, right| {
        right
            .worth
            .cmp(&left.worth)
            .then_with(|| left.offer.id().as_ref().cmp(right.offer.id().as_ref()))
    });
    ranked
}
