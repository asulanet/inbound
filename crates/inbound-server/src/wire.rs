use commonware_codec::Encode;
use inbound_types::{ChannelSpec, Offer, Predicate, Registry, Reward, Terms};
use serde::Serialize;

/// The JSON an agent receives. `bytes` is the canonical encoding the seller signed, so an agent
/// that wants to check the signature itself has everything it needs; the readable fields are a
/// convenience on top.
#[derive(Serialize)]
pub struct WireOffer {
    pub position: u64,
    pub id: String,
    pub seller: String,
    pub channel: String,
    pub format: String,
    pub reward: WireReward,
    pub better: Vec<WireTerms>,
    pub min_spend: u64,
    pub valid_from: u64,
    pub valid_until: u64,
    pub budget: u64,
    pub bond: u64,
    pub bytes: String,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireReward {
    Percent { basis_points: u16, cap: u64 },
    Fixed { amount: u64 },
}

#[derive(Serialize)]
pub struct WireTerms {
    pub predicate: WirePredicate,
    pub reward: WireReward,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WirePredicate {
    NeverBoughtFrom { seller: String },
    BudgetAtLeast { amount: u64 },
}

#[derive(Serialize)]
pub struct WireChannel {
    pub channel: String,
    pub operator: String,
    pub format: String,
    pub min_bond: u64,
    pub tip: u64,
    pub subscribers: usize,
}

#[derive(Serialize)]
pub struct WireRegistry {
    pub maintainer: String,
    pub published_at: u64,
    pub channels: Vec<WireChannelSpec>,
    pub bytes: String,
}

#[derive(Serialize)]
pub struct WireChannelSpec {
    pub channel: String,
    pub operator: String,
    pub format: String,
    pub min_bond: u64,
    pub stream: String,
    pub offers: String,
}

impl WireOffer {
    pub fn new(position: u64, offer: &Offer) -> Self {
        Self {
            position,
            id: format!("{}", offer.id()),
            seller: format!("{}", offer.body.seller),
            channel: offer.body.channel.to_string(),
            format: offer.body.format.to_string(),
            reward: reward(&offer.body.reward),
            better: offer.body.better.iter().map(terms).collect(),
            min_spend: offer.body.min_spend,
            valid_from: offer.body.valid_from,
            valid_until: offer.body.valid_until,
            budget: offer.body.budget,
            bond: offer.body.bond,
            bytes: hex::encode(offer.encode()),
        }
    }
}

fn reward(value: &Reward) -> WireReward {
    match value {
        Reward::Percent { basis_points, cap } => WireReward::Percent {
            basis_points: *basis_points,
            cap: *cap,
        },
        Reward::Fixed(amount) => WireReward::Fixed { amount: *amount },
    }
}

fn terms(value: &Terms) -> WireTerms {
    WireTerms {
        predicate: match &value.predicate {
            Predicate::NeverBoughtFrom(seller) => WirePredicate::NeverBoughtFrom {
                seller: format!("{seller}"),
            },
            Predicate::BudgetAtLeast(amount) => WirePredicate::BudgetAtLeast { amount: *amount },
        },
        reward: reward(&value.reward),
    }
}

pub fn registry(value: &Registry, base: &str) -> WireRegistry {
    WireRegistry {
        maintainer: format!("{}", value.body.maintainer),
        published_at: value.body.published_at,
        channels: value
            .body
            .channels
            .iter()
            .map(|spec| channel_spec(spec, base))
            .collect(),
        bytes: hex::encode(value.encode()),
    }
}

fn channel_spec(spec: &ChannelSpec, base: &str) -> WireChannelSpec {
    let slug = spec.channel.as_str();
    WireChannelSpec {
        channel: slug.to_string(),
        operator: format!("{}", spec.operator),
        format: spec.format.to_string(),
        min_bond: spec.min_bond,
        stream: format!("{base}/channels/{slug}/stream"),
        offers: format!("{base}/channels/{slug}/offers"),
    }
}
