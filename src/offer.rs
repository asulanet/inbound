use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Read, ReadExt, Write};
use commonware_cryptography::ed25519::{PrivateKey, PublicKey, Signature};
use commonware_cryptography::{Hasher, Signer, Verifier, sha256::Digest, sha256::Sha256};

/// Signing is namespaced so an offer signature cannot be replayed as anything else.
pub const NAMESPACE: &[u8] = b"_INBOUND_OFFER_V0";

const MAX_NAME: usize = 128;
const MAX_TERMS: usize = 4;

/// A property of the buyer a seller will give better terms for. The buyer's agent checks it
/// against its own data; nothing leaves the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Predicate {
    NeverBoughtFrom(PublicKey),
    BudgetAtLeast(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reward {
    pub basis_points: u16,
    pub cap: u64,
}

impl Reward {
    pub fn on(&self, spend: u64) -> u64 {
        (spend.saturating_mul(u64::from(self.basis_points)) / 10_000).min(self.cap)
    }
}

/// Better terms, unlocked by one predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terms {
    pub predicate: Predicate,
    pub reward: Reward,
}

/// What a seller posts. Fixed fields, no free text: there is nothing here for a seller to write
/// prose into, so there is nothing to talk an agent round with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub seller: PublicKey,
    pub channel: String,
    pub reward: Reward,
    pub better: Vec<Terms>,
    pub min_spend: u64,
    pub expires: u64,
    pub nonce: u64,
}

impl Body {
    /// The reward for a buyer holding these predicates: the best terms they qualify for.
    pub fn reward_for(&self, holds: &[Predicate]) -> Reward {
        self.better
            .iter()
            .filter(|terms| holds.contains(&terms.predicate))
            .map(|terms| terms.reward)
            .chain(std::iter::once(self.reward))
            .max_by_key(|reward| reward.cap)
            .unwrap_or(self.reward)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub body: Body,
    pub signature: Signature,
}

impl Offer {
    pub fn sign(seller: &PrivateKey, body: Body) -> Self {
        let signature = seller.sign(NAMESPACE, &body.encode());
        Self { body, signature }
    }

    /// Checked from the bytes the seller signed, so a relaying operator cannot edit a rate.
    pub fn verify(&self) -> bool {
        self.body
            .seller
            .verify(NAMESPACE, &self.body.encode(), &self.signature)
    }

    pub fn id(&self) -> Digest {
        Sha256::hash(&[NAMESPACE, &self.body.encode()])
    }

    pub fn live_at(&self, now: u64) -> bool {
        now < self.body.expires
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
        1 + match self {
            Self::NeverBoughtFrom(seller) => seller.encode_size(),
            Self::BudgetAtLeast(amount) => amount.encode_size(),
        }
    }
}

impl Read for Predicate {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        match u8::read(buf)? {
            0 => Ok(Self::NeverBoughtFrom(PublicKey::read(buf)?)),
            1 => Ok(Self::BudgetAtLeast(u64::read(buf)?)),
            tag => Err(commonware_codec::Error::InvalidEnum(tag)),
        }
    }
}

impl Write for Reward {
    fn write(&self, buf: &mut impl BufMut) {
        self.basis_points.write(buf);
        self.cap.write(buf);
    }
}

impl EncodeSize for Reward {
    fn encode_size(&self) -> usize {
        self.basis_points.encode_size() + self.cap.encode_size()
    }
}

impl Read for Reward {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            basis_points: u16::read(buf)?,
            cap: u64::read(buf)?,
        })
    }
}

impl Write for Terms {
    fn write(&self, buf: &mut impl BufMut) {
        self.predicate.write(buf);
        self.reward.write(buf);
    }
}

impl EncodeSize for Terms {
    fn encode_size(&self) -> usize {
        self.predicate.encode_size() + self.reward.encode_size()
    }
}

impl Read for Terms {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            predicate: Predicate::read(buf)?,
            reward: Reward::read(buf)?,
        })
    }
}

impl Write for Body {
    fn write(&self, buf: &mut impl BufMut) {
        self.seller.write(buf);
        self.channel.as_bytes().to_vec().write(buf);
        self.reward.write(buf);
        self.better.write(buf);
        self.min_spend.write(buf);
        self.expires.write(buf);
        self.nonce.write(buf);
    }
}

impl EncodeSize for Body {
    fn encode_size(&self) -> usize {
        self.seller.encode_size()
            + self.channel.as_bytes().to_vec().encode_size()
            + self.reward.encode_size()
            + self.better.encode_size()
            + self.min_spend.encode_size()
            + self.expires.encode_size()
            + self.nonce.encode_size()
    }
}

impl Read for Body {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        let seller = PublicKey::read(buf)?;
        let raw = Vec::<u8>::read_cfg(buf, &((1..=MAX_NAME).into(), ()))?;
        let channel = String::from_utf8(raw)
            .map_err(|_| commonware_codec::Error::Invalid("channel", "not utf-8"))?;
        Ok(Self {
            seller,
            channel,
            reward: Reward::read(buf)?,
            better: Vec::<Terms>::read_cfg(buf, &((..=MAX_TERMS).into(), ()))?,
            min_spend: u64::read(buf)?,
            expires: u64::read(buf)?,
            nonce: u64::read(buf)?,
        })
    }
}

impl Write for Offer {
    fn write(&self, buf: &mut impl BufMut) {
        self.body.write(buf);
        self.signature.write(buf);
    }
}

impl EncodeSize for Offer {
    fn encode_size(&self) -> usize {
        self.body.encode_size() + self.signature.encode_size()
    }
}

impl Read for Offer {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            body: Body::read(buf)?,
            signature: Signature::read(buf)?,
        })
    }
}
