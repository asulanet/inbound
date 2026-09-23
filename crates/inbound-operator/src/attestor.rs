use commonware_cryptography::Signer;
use inbound_types::{Predicate, PredicateProof, PrivateKey, ProofBody, PublicKey};
use std::collections::HashMap;

pub const PROOF_LIFETIME: u64 = 15 * 60;

/// What the wallet knows about one buyer from its own records.
#[derive(Clone, Debug, Default)]
pub struct BuyerFacts {
    pub available_balance: u64,
    pub settled_spend_by_seller: HashMap<PublicKey, u64>,
}

impl BuyerFacts {
    pub fn holds(&self, predicate: &Predicate) -> bool {
        match predicate {
            Predicate::NeverBoughtFrom(seller) => {
                self.settled_spend_by_seller
                    .get(seller)
                    .copied()
                    .unwrap_or(0)
                    == 0
            }
            Predicate::BudgetAtLeast(amount) => self.available_balance >= *amount,
        }
    }
}

/// Vouches for predicates about its own users. Here that is the wallet: it onboarded the buyer and
/// it sees the settlements, so it is the only party that can say whether a buyer has ever paid a
/// given seller.
pub struct Attestor {
    key: PrivateKey,
    facts: HashMap<PublicKey, BuyerFacts>,
}

impl Attestor {
    pub fn new(key: PrivateKey) -> Self {
        Self {
            key,
            facts: HashMap::new(),
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.key.public_key()
    }

    pub fn record(&mut self, buyer_root: PublicKey, facts: BuyerFacts) {
        self.facts.insert(buyer_root, facts);
    }

    pub fn record_settlement(&mut self, buyer_root: &PublicKey, seller: PublicKey, amount: u64) {
        let facts = self.facts.entry(buyer_root.clone()).or_default();
        *facts.settled_spend_by_seller.entry(seller).or_insert(0) += amount;
    }

    pub fn holds(&self, buyer_root: &PublicKey, predicate: &Predicate) -> bool {
        self.facts
            .get(buyer_root)
            .is_some_and(|facts| facts.holds(predicate))
    }

    /// Produce evidence for one predicate, bound to one seller and short-lived. Returns `None`
    /// when the predicate does not hold, so an agent cannot obtain a proof of something false.
    pub fn attest(
        &self,
        buyer_root: &PublicKey,
        subject: PublicKey,
        audience: PublicKey,
        predicate: &Predicate,
        now: u64,
    ) -> Option<PredicateProof> {
        if !self.holds(buyer_root, predicate) {
            return None;
        }
        let body = ProofBody {
            predicate: predicate.clone(),
            attestor: self.public_key(),
            subject,
            audience,
            issued_at: now,
            expires_at: now + PROOF_LIFETIME,
        };
        Some(PredicateProof::sign(&self.key, body))
    }
}
