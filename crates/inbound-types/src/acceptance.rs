use crate::{
    ACCEPTANCE_NAMESPACE, Digest, Error, MAX_PROOFS, PredicateProof, PrivateKey, PublicKey,
    Signature,
};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Read, ReadExt, Write};
use commonware_cryptography::{Hasher, Signer, Verifier, sha256::Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Nullifier(pub Digest);

impl Nullifier {
    pub fn derive(credential_secret: &[u8], campaign: &Digest) -> Self {
        Self(Sha256::hash(&[
            b"_INBOUND_NULLIFIER_V0",
            credential_secret,
            campaign.as_ref(),
        ]))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptanceBody {
    pub offer: Digest,
    pub buyer: PublicKey,
    pub proofs: Vec<PredicateProof>,
    pub nullifier: Nullifier,
    pub accepted_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acceptance {
    pub body: AcceptanceBody,
    pub signature: Signature,
}

impl Acceptance {
    pub fn sign(buyer: &PrivateKey, body: AcceptanceBody) -> Self {
        let signature = buyer.sign(ACCEPTANCE_NAMESPACE, &body.encode());
        Self { body, signature }
    }

    pub fn verify(&self) -> Result<(), Error> {
        if !self
            .body
            .buyer
            .verify(ACCEPTANCE_NAMESPACE, &self.body.encode(), &self.signature)
        {
            return Err(Error::BadSignature);
        }
        Ok(())
    }

    pub fn id(&self) -> Digest {
        Sha256::hash(&[ACCEPTANCE_NAMESPACE, &self.body.encode()])
    }
}

impl Write for Nullifier {
    fn write(&self, buf: &mut impl BufMut) {
        self.0.write(buf);
    }
}

impl EncodeSize for Nullifier {
    fn encode_size(&self) -> usize {
        self.0.encode_size()
    }
}

impl Read for Nullifier {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self(Digest::read(buf)?))
    }
}

impl Write for AcceptanceBody {
    fn write(&self, buf: &mut impl BufMut) {
        self.offer.write(buf);
        self.buyer.write(buf);
        self.proofs.write(buf);
        self.nullifier.write(buf);
        self.accepted_at.write(buf);
    }
}

impl EncodeSize for AcceptanceBody {
    fn encode_size(&self) -> usize {
        self.offer.encode_size()
            + self.buyer.encode_size()
            + self.proofs.encode_size()
            + self.nullifier.encode_size()
            + self.accepted_at.encode_size()
    }
}

impl Read for AcceptanceBody {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            offer: Digest::read(buf)?,
            buyer: PublicKey::read(buf)?,
            proofs: Vec::<PredicateProof>::read_cfg(buf, &((..=MAX_PROOFS).into(), ()))?,
            nullifier: Nullifier::read(buf)?,
            accepted_at: u64::read(buf)?,
        })
    }
}

impl Write for Acceptance {
    fn write(&self, buf: &mut impl BufMut) {
        self.body.write(buf);
        self.signature.write(buf);
    }
}

impl EncodeSize for Acceptance {
    fn encode_size(&self) -> usize {
        self.body.encode_size() + self.signature.encode_size()
    }
}

impl Read for Acceptance {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            body: AcceptanceBody::read(buf)?,
            signature: Signature::read(buf)?,
        })
    }
}
