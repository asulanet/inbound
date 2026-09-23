use crate::Error;
use inbound_types::{Acceptance, Digest, Nullifier, Offer, Predicate, PublicKey};
use std::collections::{HashMap, HashSet};

pub const PENDING_PERIOD: u64 = 30 * 24 * 60 * 60;

/// A payment the operator saw itself, because it issued the card or the rail the buyer paid on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settlement {
    pub seller: PublicKey,
    pub buyer_root: PublicKey,
    pub amount: u64,
    pub settled_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sale {
    pub acceptance: Digest,
    pub offer: Digest,
    pub reward: u64,
    pub operator_fee: u64,
    pub payable_at: u64,
}

struct Reservation {
    offer: Digest,
    buyer_root: PublicKey,
    proven: Vec<Predicate>,
    reserved: u64,
}

/// Tracks what each posted offer can still pay, which acceptances are outstanding, and which
/// settlements turned into sales.
pub struct Ledger {
    attestor: PublicKey,
    flat_fee: u64,
    offers: HashMap<Digest, Offer>,
    remaining_budget: HashMap<Digest, u64>,
    used_nullifiers: HashSet<(Digest, Nullifier)>,
    reservations: HashMap<Digest, Reservation>,
    sales: Vec<Sale>,
}

impl Ledger {
    pub fn new(attestor: PublicKey, flat_fee: u64) -> Self {
        Self {
            attestor,
            flat_fee,
            offers: HashMap::new(),
            remaining_budget: HashMap::new(),
            used_nullifiers: HashSet::new(),
            reservations: HashMap::new(),
            sales: Vec::new(),
        }
    }

    pub fn fund(&mut self, offer: &Offer) {
        let id = offer.id();
        self.remaining_budget.insert(id, offer.body.budget);
        self.offers.insert(id, offer.clone());
    }

    pub fn remaining_budget(&self, offer: &Digest) -> u64 {
        self.remaining_budget.get(offer).copied().unwrap_or(0)
    }

    /// Step 4 of the protocol. Checks the acceptance and every proof attached to it, takes the
    /// worst-case reward out of the offer's budget, and burns the nullifier so the same person
    /// cannot take this offer twice.
    pub fn reserve(
        &mut self,
        acceptance: Acceptance,
        buyer_root: PublicKey,
        now: u64,
    ) -> Result<Digest, Error> {
        acceptance
            .verify()
            .map_err(|error| Error::Rejected(error.to_string()))?;
        let offer = self
            .offers
            .get(&acceptance.body.offer)
            .ok_or(Error::UnknownOffer)?
            .clone();
        if !offer.is_live_at(now) {
            return Err(Error::Rejected("offer is not live".into()));
        }
        let proven = self.check_proofs(&offer, &acceptance, now)?;

        let key = (acceptance.body.offer, acceptance.body.nullifier);
        if self.used_nullifiers.contains(&key) {
            return Err(Error::NullifierUsed);
        }
        let reserved = offer.body.reward_for(&proven).maximum();
        let remaining = self.remaining_budget(&acceptance.body.offer);
        if remaining < reserved {
            return Err(Error::BudgetExhausted);
        }
        self.remaining_budget
            .insert(acceptance.body.offer, remaining - reserved);
        self.used_nullifiers.insert(key);

        let id = acceptance.id();
        self.reservations.insert(
            id,
            Reservation {
                offer: offer.id(),
                buyer_root,
                proven,
                reserved,
            },
        );
        Ok(id)
    }

    /// Matches a payment the operator observed against an outstanding reservation. This is the
    /// only event that bills the seller.
    pub fn settle(&mut self, settlement: &Settlement) -> Option<Sale> {
        let matched = self.reservations.iter().find_map(|(id, reservation)| {
            let offer = self.offers.get(&reservation.offer)?;
            let fits = offer.body.seller == settlement.seller
                && reservation.buyer_root == settlement.buyer_root
                && settlement.amount >= offer.body.min_spend
                && offer.is_live_at(settlement.settled_at);
            fits.then_some(*id)
        })?;
        let reservation = self.reservations.remove(&matched)?;
        let offer = self.offers.get(&reservation.offer)?;
        let reward = offer
            .body
            .reward_for(&reservation.proven)
            .value_for(settlement.amount);
        if let Some(remaining) = self.remaining_budget.get_mut(&reservation.offer) {
            *remaining += reservation.reserved.saturating_sub(reward);
        }
        let sale = Sale {
            acceptance: matched,
            offer: reservation.offer,
            reward,
            operator_fee: self.flat_fee,
            payable_at: settlement.settled_at + PENDING_PERIOD,
        };
        self.sales.push(sale.clone());
        Some(sale)
    }

    pub fn sales(&self) -> &[Sale] {
        &self.sales
    }

    fn check_proofs(
        &self,
        offer: &Offer,
        acceptance: &Acceptance,
        now: u64,
    ) -> Result<Vec<Predicate>, Error> {
        let mut proven = Vec::with_capacity(acceptance.body.proofs.len());
        for proof in &acceptance.body.proofs {
            if proof.body.subject != acceptance.body.buyer {
                return Err(Error::Rejected("proof is about a different buyer".into()));
            }
            if !offer
                .body
                .better
                .iter()
                .any(|terms| terms.predicate == proof.body.predicate)
            {
                return Err(Error::Rejected(
                    "proof for a predicate this offer does not price".into(),
                ));
            }
            proof
                .verify(&self.attestor, &offer.body.seller, now)
                .map_err(|error| Error::Rejected(error.to_string()))?;
            proven.push(proof.body.predicate.clone());
        }
        Ok(proven)
    }
}
