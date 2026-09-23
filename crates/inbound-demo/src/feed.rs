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
    /// Roughly how many days between this seller's offers.
    pub cadence: u64,
    /// Basis points this seller is willing to give anyone.
    pub base: u16,
    /// Extra basis points for a buyer who has never bought from them, if they bid for new
    /// customers at all.
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
            cadence: 9,
            base: 800,
            new_customer_bonus: Some(1_200),
        },
        Seller {
            name: "trailco",
            key: PrivateKey::from_seed(11),
            cadence: 6,
            base: 500,
            new_customer_bonus: None,
        },
        Seller {
            name: "peakform",
            key: PrivateKey::from_seed(12),
            cadence: 13,
            base: 1_100,
            new_customer_bonus: Some(900),
        },
        Seller {
            name: "cadence",
            key: PrivateKey::from_seed(13),
            cadence: 8,
            base: 400,
            new_customer_bonus: None,
        },
    ]
}

/// One offer landing in the channel on a given day.
pub struct Posting {
    pub day: u64,
    pub seller_index: usize,
    pub offer: Offer,
}

/// Builds a month of plausible traffic: each seller posts on its own rough cadence, with rates
/// that wobble, minimum spends that vary, and windows that expire part-way through.
pub fn month_of_traffic(
    sellers: &[Seller],
    channel: &ChannelId,
    format: &FormatId,
    start: u64,
    days: u64,
    seed: u64,
) -> Vec<Posting> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut postings = Vec::new();
    let min_spends = [3_000u64, 4_000, 6_000, 8_000];

    for (seller_index, seller) in sellers.iter().enumerate() {
        let mut day = rng.random_range(0..seller.cadence.min(days));
        let mut nonce = 0u64;
        while day < days {
            nonce += 1;
            let wobble = rng.random_range(0..=300u16);
            let base = seller.base.saturating_add(wobble);
            let lifetime = rng.random_range(5..=14u64);
            let min_spend = *min_spends.choose(&mut rng).unwrap_or(&4_000);

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
                valid_from: start + day * DAY,
                valid_until: start + (day + lifetime).min(days) * DAY,
                budget: 500_000,
                nonce,
            };
            postings.push(Posting {
                day,
                seller_index,
                offer: Offer::sign(&seller.key, body),
            });

            let gap =
                rng.random_range(seller.cadence.saturating_sub(3).max(2)..=seller.cadence + 4);
            day += gap;
        }
    }

    postings.sort_by_key(|posting| (posting.day, posting.seller_index));
    postings
}
