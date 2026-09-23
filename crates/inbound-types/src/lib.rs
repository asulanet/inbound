mod acceptance;
mod codec;
mod offer;
mod predicate;
mod registry;

pub use acceptance::{Acceptance, AcceptanceBody, Nullifier};
pub use codec::{ChannelId, FormatId};
pub use commonware_cryptography::ed25519::{PrivateKey, PublicKey, Signature};
pub use commonware_cryptography::sha256::Digest;
pub use offer::{Offer, OfferBody, Reward, Terms};
pub use predicate::{Predicate, PredicateProof, ProofBody};
pub use registry::{ChannelSpec, Registry, RegistryBody};

pub const OFFER_NAMESPACE: &[u8] = b"_INBOUND_OFFER_V0";
pub const PROOF_NAMESPACE: &[u8] = b"_INBOUND_PROOF_V0";
pub const ACCEPTANCE_NAMESPACE: &[u8] = b"_INBOUND_ACCEPTANCE_V0";
pub const REGISTRY_NAMESPACE: &[u8] = b"_INBOUND_REGISTRY_V0";

pub const MAX_NAME_BYTES: usize = 128;
pub const MAX_TERMS: usize = 8;
pub const MAX_PROOFS: usize = 8;
pub const MAX_CHANNELS: usize = 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("signature does not verify")]
    BadSignature,
    #[error("decode: {0}")]
    Decode(String),
}

impl From<commonware_codec::Error> for Error {
    fn from(error: commonware_codec::Error) -> Self {
        Self::Decode(error.to_string())
    }
}
