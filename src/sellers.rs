//! Made-up shops, so the channels have something in them.

use crate::{Body, Offer, Predicate, Reward, Terms};
use commonware_cryptography::Signer;
use commonware_cryptography::ed25519::{PrivateKey, PublicKey};
use rand::RngExt;
use rand_chacha::ChaCha20Rng;

/// The channels this operator runs. An agent finds these through the registry.
pub const CHANNELS: &[&str] = &["cashback/us/sportswear", "cashback/us/coffee", "cars/ev/us"];

pub struct Seller {
    pub name: &'static str,
    pub channel: &'static str,
    pub key: PrivateKey,
    /// What it gives anyone, in basis points.
    pub base: u16,
    /// Extra basis points for someone who has never bought from it, if it bids for newcomers.
    pub for_newcomers: Option<u16>,
    /// The smallest basket it will pay out on, in cents.
    pub min_spend: u64,
}

impl Seller {
    pub fn public_key(&self) -> PublicKey {
        self.key.public_key()
    }
}

pub fn sellers() -> Vec<Seller> {
    vec![
        // Sportswear: crowded, everyone discounts, two chase new customers hard.
        Seller {
            name: "runfast",
            channel: "cashback/us/sportswear",
            key: PrivateKey::from_seed(10),
            base: 800,
            for_newcomers: Some(1_200),
            min_spend: 6_000,
        },
        Seller {
            name: "trailco",
            channel: "cashback/us/sportswear",
            key: PrivateKey::from_seed(11),
            base: 500,
            for_newcomers: None,
            min_spend: 4_000,
        },
        Seller {
            name: "peakform",
            channel: "cashback/us/sportswear",
            key: PrivateKey::from_seed(12),
            base: 1_100,
            for_newcomers: Some(900),
            min_spend: 8_000,
        },
        Seller {
            name: "cadence",
            channel: "cashback/us/sportswear",
            key: PrivateKey::from_seed(13),
            base: 400,
            for_newcomers: None,
            min_spend: 3_000,
        },
        Seller {
            name: "stride",
            channel: "cashback/us/sportswear",
            key: PrivateKey::from_seed(14),
            base: 650,
            for_newcomers: Some(600),
            min_spend: 5_000,
        },
        // Coffee: small baskets, thin margins, nobody bids much for newcomers.
        Seller {
            name: "blackbird",
            channel: "cashback/us/coffee",
            key: PrivateKey::from_seed(20),
            base: 300,
            for_newcomers: Some(400),
            min_spend: 1_500,
        },
        Seller {
            name: "kettleworks",
            channel: "cashback/us/coffee",
            key: PrivateKey::from_seed(21),
            base: 450,
            for_newcomers: None,
            min_spend: 2_000,
        },
        Seller {
            name: "ninthstreet",
            channel: "cashback/us/coffee",
            key: PrivateKey::from_seed(22),
            base: 250,
            for_newcomers: None,
            min_spend: 1_000,
        },
        // Cars: rare, enormous, and the newcomer premium is where the money is.
        Seller {
            name: "voltline",
            channel: "cars/ev/us",
            key: PrivateKey::from_seed(30),
            base: 50,
            for_newcomers: Some(150),
            min_spend: 2_500_000,
        },
        Seller {
            name: "meridian-ev",
            channel: "cars/ev/us",
            key: PrivateKey::from_seed(31),
            base: 80,
            for_newcomers: None,
            min_spend: 3_000_000,
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

/// One plausible offer from one seller: rates wobble, windows are short.
pub fn write_offer(rng: &mut ChaCha20Rng, seller: &Seller, now: u64, nonce: u64) -> Offer {
    let base = seller.base.saturating_add(rng.random_range(0..=400u16));
    let cap = seller.min_spend.saturating_mul(3) / 10;

    let better = seller
        .for_newcomers
        .map(|bonus| {
            vec![Terms {
                predicate: Predicate::NeverBoughtFrom(seller.public_key()),
                reward: Reward {
                    basis_points: base.saturating_add(bonus),
                    cap: cap.saturating_mul(2),
                },
            }]
        })
        .unwrap_or_default();

    Offer::sign(
        &seller.key,
        Body {
            seller: seller.public_key(),
            channel: seller.channel.to_string(),
            reward: Reward {
                basis_points: base,
                cap,
            },
            better,
            min_spend: seller.min_spend,
            expires: now + rng.random_range(4..=12u64) * 86_400,
            nonce,
        },
    )
}

/// Picks whoever posts next. Busier channels post more often, which is what makes the registry
/// worth reading rather than guessing.
pub fn next_seller(rng: &mut ChaCha20Rng, sellers: &[Seller]) -> usize {
    rng.random_range(0..sellers.len())
}
