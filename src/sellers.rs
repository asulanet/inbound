//! Four made-up shoe shops. They exist so the channel has something in it.

use crate::{Body, Offer, Predicate, Reward, Terms};
use commonware_cryptography::Signer;
use commonware_cryptography::ed25519::{PrivateKey, PublicKey};
use rand::RngExt;
use rand_chacha::ChaCha20Rng;

pub struct Seller {
    pub name: &'static str,
    pub key: PrivateKey,
    /// What it gives anyone, in basis points.
    pub base: u16,
    /// Extra basis points for someone who has never bought from it, if it bids for new customers.
    pub for_newcomers: Option<u16>,
}

impl Seller {
    pub fn public_key(&self) -> PublicKey {
        self.key.public_key()
    }
}

pub fn sellers() -> Vec<Seller> {
    vec![
        Seller {
            name: "runfast",
            key: PrivateKey::from_seed(10),
            base: 800,
            for_newcomers: Some(1_200),
        },
        Seller {
            name: "trailco",
            key: PrivateKey::from_seed(11),
            base: 500,
            for_newcomers: None,
        },
        Seller {
            name: "peakform",
            key: PrivateKey::from_seed(12),
            base: 1_100,
            for_newcomers: Some(900),
        },
        Seller {
            name: "cadence",
            key: PrivateKey::from_seed(13),
            base: 400,
            for_newcomers: None,
        },
    ]
}

pub fn name_of(key: &PublicKey) -> String {
    sellers()
        .into_iter()
        .find(|seller| &seller.public_key() == key)
        .map(|seller| seller.name.to_string())
        .unwrap_or_else(|| format!("{key}").chars().take(8).collect())
}

/// One plausible offer: rates wobble, minimum spends differ, windows are short.
pub fn write_offer(rng: &mut ChaCha20Rng, channel: &str, now: u64, nonce: u64) -> (usize, Offer) {
    let sellers = sellers();
    let index = rng.random_range(0..sellers.len());
    let seller = &sellers[index];
    let base = seller.base.saturating_add(rng.random_range(0..=400u16));
    let min_spend = [3_000u64, 4_000, 6_000, 8_000][rng.random_range(0..4)];

    let better = seller
        .for_newcomers
        .map(|bonus| {
            vec![Terms {
                predicate: Predicate::NeverBoughtFrom(seller.public_key()),
                reward: Reward {
                    basis_points: base.saturating_add(bonus),
                    cap: 4_000,
                },
            }]
        })
        .unwrap_or_default();

    let offer = Offer::sign(
        &seller.key,
        Body {
            seller: seller.public_key(),
            channel: channel.to_string(),
            reward: Reward {
                basis_points: base,
                cap: 2_500,
            },
            better,
            min_spend,
            expires: now + rng.random_range(4..=12u64) * 86_400,
            nonce,
        },
    );
    (index, offer)
}
