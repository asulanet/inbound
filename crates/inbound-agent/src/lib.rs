mod rank;

pub use rank::{Ranked, rank};

use commonware_codec::DecodeExt;
use commonware_cryptography::{Hasher, Signer, sha256::Sha256};
use inbound_types::{
    Acceptance, AcceptanceBody, ChannelId, ChannelSpec, Nullifier, Offer, Predicate,
    PredicateProof, PrivateKey, PublicKey, Registry,
};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("registry does not verify")]
    BadRegistry,
    #[error("no channel named {0} in the registry")]
    NoSuchChannel(ChannelId),
    #[error("pairwise key derivation failed")]
    KeyDerivation,
}

/// The buyer's side: the facts their wallet holds about them, and the keys that keep those facts
/// from being linked across sellers.
#[derive(Clone, Debug)]
pub struct Buyer {
    pub root: PrivateKey,
    pub credential_secret: [u8; 32],
    pub available_balance: u64,
    pub spend_by_seller: HashMap<PublicKey, u64>,
    pub typical_basket: u64,
}

impl Buyer {
    pub fn root_public_key(&self) -> PublicKey {
        self.root.public_key()
    }

    /// A key used with exactly one seller, so two sellers cannot tell they met the same buyer.
    pub fn pairwise_key(&self, seller: &PublicKey) -> Result<PrivateKey, Error> {
        let digest = Sha256::hash(&[
            b"_INBOUND_PAIRWISE_V0",
            &self.credential_secret,
            seller.as_ref(),
        ]);
        PrivateKey::decode(digest.as_ref()).map_err(|_| Error::KeyDerivation)
    }

    /// Whether the buyer can honestly claim a predicate, checked locally against their own data.
    /// Nothing leaves the agent here.
    pub fn holds(&self, predicate: &Predicate) -> bool {
        match predicate {
            Predicate::NeverBoughtFrom(seller) => {
                self.spend_by_seller.get(seller).copied().unwrap_or(0) == 0
            }
            Predicate::BudgetAtLeast(amount) => self.available_balance >= *amount,
        }
    }

    /// Which of an offer's improved terms this buyer qualifies for.
    pub fn qualifies_for(&self, offer: &Offer) -> Vec<Predicate> {
        offer
            .body
            .better
            .iter()
            .map(|terms| terms.predicate.clone())
            .filter(|predicate| self.holds(predicate))
            .collect()
    }

    /// Step 4. Ask the attestor for evidence of each predicate the buyer qualifies for, then sign
    /// the acceptance under the per-seller key.
    pub fn accept<F>(&self, offer: &Offer, now: u64, mut prove: F) -> Result<Acceptance, Error>
    where
        F: FnMut(&PublicKey, &PublicKey, &Predicate) -> Option<PredicateProof>,
    {
        let pairwise = self.pairwise_key(&offer.body.seller)?;
        let subject = pairwise.public_key();
        let proofs = self
            .qualifies_for(offer)
            .into_iter()
            .filter_map(|predicate| prove(&subject, &offer.body.seller, &predicate))
            .collect();
        let offer_id = offer.id();
        let body = AcceptanceBody {
            offer: offer_id,
            buyer: subject,
            proofs,
            nullifier: Nullifier::derive(&self.credential_secret, &offer_id),
            accepted_at: now,
        };
        Ok(Acceptance::sign(&pairwise, body))
    }
}

/// Step 1. Look a channel up in the registry to learn who operates it and what format its offers
/// take, before posting to it or subscribing to it.
pub fn find_channel(
    registry: &Registry,
    trusted_maintainer: &PublicKey,
    channel: &ChannelId,
) -> Result<ChannelSpec, Error> {
    registry
        .verify(trusted_maintainer)
        .map_err(|_| Error::BadRegistry)?;
    registry
        .lookup(channel)
        .cloned()
        .ok_or_else(|| Error::NoSuchChannel(channel.clone()))
}

/// Where a subscribed agent has read up to in one channel.
#[derive(Clone, Debug, Default)]
pub struct Cursor(pub u64);

impl Cursor {
    pub fn advance(&mut self, delivered: &[(u64, Offer)]) {
        if let Some((position, _)) = delivered.last() {
            self.0 = position + 1;
        }
    }
}
