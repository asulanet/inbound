use crate::Buyer;
use inbound_types::{Digest, Offer};
use rand::seq::SliceRandom;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

#[derive(Clone, Debug)]
pub struct Ranked {
    pub offer: Offer,
    pub expected_reward: u64,
}

/// Ranking runs on the buyer's side, over the offer's typed fields only. Offers are shuffled
/// before scoring and ties break on the offer id, so neither the order they arrived in nor
/// anything a seller writes can influence the result.
pub fn rank(offers: Vec<Offer>, buyer: &Buyer, now: u64, shuffle_seed: u64) -> Vec<Ranked> {
    let mut live: Vec<Offer> = offers
        .into_iter()
        .filter(|offer| offer.verify().is_ok())
        .filter(|offer| offer.is_live_at(now))
        .filter(|offer| buyer.typical_basket >= offer.body.min_spend)
        .collect();

    let mut rng = ChaCha20Rng::seed_from_u64(shuffle_seed);
    live.shuffle(&mut rng);

    let mut ranked: Vec<Ranked> = live
        .into_iter()
        .map(|offer| {
            let qualified = buyer.qualifies_for(&offer);
            let expected_reward = offer
                .body
                .reward_for(&qualified)
                .value_for(buyer.typical_basket);
            Ranked {
                offer,
                expected_reward,
            }
        })
        .collect();

    ranked.sort_by(|left, right| {
        right
            .expected_reward
            .cmp(&left.expected_reward)
            .then_with(|| tie_break(&left.offer.id(), &right.offer.id()))
    });
    ranked
}

fn tie_break(left: &Digest, right: &Digest) -> std::cmp::Ordering {
    left.as_ref().cmp(right.as_ref())
}
