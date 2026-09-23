use crate::codec::{read_tag, unknown_tag};
use crate::{Error, PROOF_NAMESPACE, PrivateKey, PublicKey, Signature};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Read, ReadExt, Write};
use commonware_cryptography::{Signer, Verifier};

/// A property of the buyer that a seller will give better terms for. The buyer's agent evaluates
/// it locally against facts the wallet already holds, and proves it only when accepting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Predicate {
    /// The buyer has never paid this seller from this wallet.
    NeverBoughtFrom(PublicKey),
    /// The buyer's available balance is at or above this amount.
    BudgetAtLeast(u64),
}

/// Evidence that a predicate holds.
///
/// Today this is an attestation signed by the wallet that holds the underlying facts, bound to one
/// seller so two sellers cannot link the same buyer. The post describes a zero-knowledge proof in
/// this position; the shape is the same (the seller checks the predicate, learns nothing else) and
/// swapping the construction does not change the protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofBody {
    pub predicate: Predicate,
    pub attestor: PublicKey,
    pub subject: PublicKey,
    pub audience: PublicKey,
    pub issued_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateProof {
    pub body: ProofBody,
    pub signature: Signature,
}

impl PredicateProof {
    pub fn sign(attestor: &PrivateKey, body: ProofBody) -> Self {
        let signature = attestor.sign(PROOF_NAMESPACE, &body.encode());
        Self { body, signature }
    }

    pub fn verify(
        &self,
        trusted_attestor: &PublicKey,
        audience: &PublicKey,
        now: u64,
    ) -> Result<(), Error> {
        if &self.body.attestor != trusted_attestor || &self.body.audience != audience {
            return Err(Error::BadSignature);
        }
        if now < self.body.issued_at || now >= self.body.expires_at {
            return Err(Error::BadSignature);
        }
        if !self
            .body
            .attestor
            .verify(PROOF_NAMESPACE, &self.body.encode(), &self.signature)
        {
            return Err(Error::BadSignature);
        }
        Ok(())
    }
}

impl Write for Predicate {
    fn write(&self, buf: &mut impl BufMut) {
        match self {
            Self::NeverBoughtFrom(seller) => {
                0u8.write(buf);
                seller.write(buf);
            }
            Self::BudgetAtLeast(amount) => {
                1u8.write(buf);
                amount.write(buf);
            }
        }
    }
}

impl EncodeSize for Predicate {
    fn encode_size(&self) -> usize {
        match self {
            Self::NeverBoughtFrom(seller) => 1 + seller.encode_size(),
            Self::BudgetAtLeast(amount) => 1 + amount.encode_size(),
        }
    }
}

impl Read for Predicate {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        match read_tag(buf)? {
            0 => Ok(Self::NeverBoughtFrom(PublicKey::read(buf)?)),
            1 => Ok(Self::BudgetAtLeast(u64::read(buf)?)),
            tag => Err(unknown_tag(tag)),
        }
    }
}

impl Write for ProofBody {
    fn write(&self, buf: &mut impl BufMut) {
        self.predicate.write(buf);
        self.attestor.write(buf);
        self.subject.write(buf);
        self.audience.write(buf);
        self.issued_at.write(buf);
        self.expires_at.write(buf);
    }
}

impl EncodeSize for ProofBody {
    fn encode_size(&self) -> usize {
        self.predicate.encode_size()
            + self.attestor.encode_size()
            + self.subject.encode_size()
            + self.audience.encode_size()
            + self.issued_at.encode_size()
            + self.expires_at.encode_size()
    }
}

impl Read for ProofBody {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            predicate: Predicate::read(buf)?,
            attestor: PublicKey::read(buf)?,
            subject: PublicKey::read(buf)?,
            audience: PublicKey::read(buf)?,
            issued_at: u64::read(buf)?,
            expires_at: u64::read(buf)?,
        })
    }
}

impl Write for PredicateProof {
    fn write(&self, buf: &mut impl BufMut) {
        self.body.write(buf);
        self.signature.write(buf);
    }
}

impl EncodeSize for PredicateProof {
    fn encode_size(&self) -> usize {
        self.body.encode_size() + self.signature.encode_size()
    }
}

impl Read for PredicateProof {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            body: ProofBody::read(buf)?,
            signature: Signature::read(buf)?,
        })
    }
}
