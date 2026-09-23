use crate::codec::{read_tag, unknown_tag};
use crate::{
    ChannelId, Digest, Error, FormatId, MAX_TERMS, OFFER_NAMESPACE, Predicate, PrivateKey,
    PublicKey, Signature,
};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Read, ReadExt, Write};
use commonware_cryptography::{Hasher, Signer, Verifier, sha256::Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reward {
    Percent { basis_points: u16, cap: u64 },
    Fixed(u64),
}

impl Reward {
    pub fn value_for(&self, spend: u64) -> u64 {
        match self {
            Self::Percent { basis_points, cap } => {
                let raw = spend.saturating_mul(u64::from(*basis_points)) / 10_000;
                raw.min(*cap)
            }
            Self::Fixed(amount) => *amount,
        }
    }

    pub fn maximum(&self) -> u64 {
        match self {
            Self::Percent { cap, .. } => *cap,
            Self::Fixed(amount) => *amount,
        }
    }
}

/// Better terms the seller will give a buyer who can prove the predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terms {
    pub predicate: Predicate,
    pub reward: Reward,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferBody {
    pub seller: PublicKey,
    pub channel: ChannelId,
    pub format: FormatId,
    pub reward: Reward,
    /// Improved terms, each unlocked by proving one predicate. Empty for a plain offer.
    pub better: Vec<Terms>,
    pub min_spend: u64,
    pub valid_from: u64,
    pub valid_until: u64,
    pub budget: u64,
    pub nonce: u64,
}

impl OfferBody {
    /// The reward for a buyer who proved `proven`, taking the best terms they qualify for.
    pub fn reward_for(&self, proven: &[Predicate]) -> Reward {
        self.better
            .iter()
            .filter(|terms| proven.contains(&terms.predicate))
            .map(|terms| terms.reward)
            .chain(std::iter::once(self.reward))
            .max_by_key(|reward| reward.maximum())
            .unwrap_or(self.reward)
    }

    /// The most this offer can pay one buyer, which is what an acceptance reserves.
    pub fn max_reward(&self) -> u64 {
        self.better
            .iter()
            .map(|terms| terms.reward.maximum())
            .chain(std::iter::once(self.reward.maximum()))
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub body: OfferBody,
    pub signature: Signature,
}

impl Offer {
    pub fn sign(seller: &PrivateKey, body: OfferBody) -> Self {
        let signature = seller.sign(OFFER_NAMESPACE, &body.encode());
        Self { body, signature }
    }

    pub fn verify(&self) -> Result<(), Error> {
        if !self
            .body
            .seller
            .verify(OFFER_NAMESPACE, &self.body.encode(), &self.signature)
        {
            return Err(Error::BadSignature);
        }
        Ok(())
    }

    pub fn id(&self) -> Digest {
        Sha256::hash(&[OFFER_NAMESPACE, &self.body.encode()])
    }

    pub fn is_live_at(&self, now: u64) -> bool {
        self.body.valid_from <= now && now < self.body.valid_until
    }
}

impl Write for Reward {
    fn write(&self, buf: &mut impl BufMut) {
        match self {
            Self::Percent { basis_points, cap } => {
                0u8.write(buf);
                basis_points.write(buf);
                cap.write(buf);
            }
            Self::Fixed(amount) => {
                1u8.write(buf);
                amount.write(buf);
            }
        }
    }
}

impl EncodeSize for Reward {
    fn encode_size(&self) -> usize {
        match self {
            Self::Percent { basis_points, cap } => {
                1 + basis_points.encode_size() + cap.encode_size()
            }
            Self::Fixed(amount) => 1 + amount.encode_size(),
        }
    }
}

impl Read for Reward {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        match read_tag(buf)? {
            0 => Ok(Self::Percent {
                basis_points: u16::read(buf)?,
                cap: u64::read(buf)?,
            }),
            1 => Ok(Self::Fixed(u64::read(buf)?)),
            tag => Err(unknown_tag(tag)),
        }
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

impl Write for OfferBody {
    fn write(&self, buf: &mut impl BufMut) {
        self.seller.write(buf);
        self.channel.write(buf);
        self.format.write(buf);
        self.reward.write(buf);
        self.better.write(buf);
        self.min_spend.write(buf);
        self.valid_from.write(buf);
        self.valid_until.write(buf);
        self.budget.write(buf);
        self.nonce.write(buf);
    }
}

impl EncodeSize for OfferBody {
    fn encode_size(&self) -> usize {
        self.seller.encode_size()
            + self.channel.encode_size()
            + self.format.encode_size()
            + self.reward.encode_size()
            + self.better.encode_size()
            + self.min_spend.encode_size()
            + self.valid_from.encode_size()
            + self.valid_until.encode_size()
            + self.budget.encode_size()
            + self.nonce.encode_size()
    }
}

impl Read for OfferBody {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            seller: PublicKey::read(buf)?,
            channel: ChannelId::read(buf)?,
            format: FormatId::read(buf)?,
            reward: Reward::read(buf)?,
            better: Vec::<Terms>::read_cfg(buf, &((..=MAX_TERMS).into(), ()))?,
            min_spend: u64::read(buf)?,
            valid_from: u64::read(buf)?,
            valid_until: u64::read(buf)?,
            budget: u64::read(buf)?,
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
            body: OfferBody::read(buf)?,
            signature: Signature::read(buf)?,
        })
    }
}
