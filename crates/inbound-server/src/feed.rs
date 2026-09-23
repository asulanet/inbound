use commonware_cryptography::Signer;
use inbound_types::{
    ChannelId, FormatId, Offer, OfferBody, Predicate, PrivateKey, PublicKey, Reward, Terms,
};
use rand::RngExt;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

pub const DAY: u64 = 24 * 60 * 60;

pub struct Seller {
    pub name: &'static str,
    pub key: PrivateKey,
    pub base: u16,
    pub new_customer_bonus: Option<u16>,
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
            new_customer_bonus: Some(1_200),
        },
        Seller {
            name: "trailco",
            key: PrivateKey::from_seed(11),
            base: 500,
            new_customer_bonus: None,
        },
        Seller {
            name: "peakform",
            key: PrivateKey::from_seed(12),
            base: 1_100,
            new_customer_bonus: Some(900),
        },
        Seller {
            name: "cadence",
            key: PrivateKey::from_seed(13),
            base: 400,
            new_customer_bonus: None,
        },
    ]
}

/// Produces one plausible offer. Fictional sellers, but the offers vary the way real promotions
/// do: rates wobble, minimum spends differ, windows are short.
pub fn next_offer(
    rng: &mut ChaCha20Rng,
    sellers: &[Seller],
    channel: &ChannelId,
    format: &FormatId,
    now: u64,
    nonce: u64,
) -> (usize, Offer) {
    let index = rng.random_range(0..sellers.len());
    let seller = &sellers[index];
    let wobble = rng.random_range(0..=400u16);
    let base = seller.base.saturating_add(wobble);
    let lifetime = rng.random_range(4..=12u64);
    let min_spends = [3_000u64, 4_000, 6_000, 8_000];
    let min_spend = *min_spends.choose(rng).unwrap_or(&4_000);

    let better = match seller.new_customer_bonus {
        Some(bonus) => vec![Terms {
            predicate: Predicate::NeverBoughtFrom(seller.public_key()),
            reward: Reward::Percent {
                basis_points: base.saturating_add(bonus),
                cap: 4_000,
            },
        }],
        None => Vec::new(),
    };

    let body = OfferBody {
        seller: seller.public_key(),
        channel: channel.clone(),
        format: format.clone(),
        reward: Reward::Percent {
            basis_points: base,
            cap: 2_500,
        },
        better,
        min_spend,
        valid_from: now,
        valid_until: now + lifetime * DAY,
        budget: 500_000,
        bond: 50_000,
        nonce,
    };
    (index, Offer::sign(&seller.key, body))
}

pub fn rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}
