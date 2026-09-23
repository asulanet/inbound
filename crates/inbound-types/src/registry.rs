use crate::{
    ChannelId, Error, FormatId, MAX_CHANNELS, PrivateKey, PublicKey, REGISTRY_NAMESPACE, Signature,
};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Read, ReadExt, Write};
use commonware_cryptography::{Signer, Verifier};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelSpec {
    pub channel: ChannelId,
    pub operator: PublicKey,
    pub format: FormatId,
    pub min_bond: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryBody {
    pub maintainer: PublicKey,
    pub channels: Vec<ChannelSpec>,
    pub published_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registry {
    pub body: RegistryBody,
    pub signature: Signature,
}

impl Registry {
    pub fn sign(maintainer: &PrivateKey, body: RegistryBody) -> Self {
        let signature = maintainer.sign(REGISTRY_NAMESPACE, &body.encode());
        Self { body, signature }
    }

    pub fn verify(&self, trusted: &PublicKey) -> Result<(), Error> {
        if &self.body.maintainer != trusted {
            return Err(Error::BadSignature);
        }
        if !self
            .body
            .maintainer
            .verify(REGISTRY_NAMESPACE, &self.body.encode(), &self.signature)
        {
            return Err(Error::BadSignature);
        }
        Ok(())
    }

    pub fn lookup(&self, channel: &ChannelId) -> Option<&ChannelSpec> {
        self.body
            .channels
            .iter()
            .find(|spec| &spec.channel == channel)
    }
}

impl Write for ChannelSpec {
    fn write(&self, buf: &mut impl BufMut) {
        self.channel.write(buf);
        self.operator.write(buf);
        self.format.write(buf);
        self.min_bond.write(buf);
    }
}

impl EncodeSize for ChannelSpec {
    fn encode_size(&self) -> usize {
        self.channel.encode_size()
            + self.operator.encode_size()
            + self.format.encode_size()
            + self.min_bond.encode_size()
    }
}

impl Read for ChannelSpec {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            channel: ChannelId::read(buf)?,
            operator: PublicKey::read(buf)?,
            format: FormatId::read(buf)?,
            min_bond: u64::read(buf)?,
        })
    }
}

impl Write for RegistryBody {
    fn write(&self, buf: &mut impl BufMut) {
        self.maintainer.write(buf);
        self.channels.write(buf);
        self.published_at.write(buf);
    }
}

impl EncodeSize for RegistryBody {
    fn encode_size(&self) -> usize {
        self.maintainer.encode_size()
            + self.channels.encode_size()
            + self.published_at.encode_size()
    }
}

impl Read for RegistryBody {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            maintainer: PublicKey::read(buf)?,
            channels: Vec::<ChannelSpec>::read_cfg(buf, &((..=MAX_CHANNELS).into(), ()))?,
            published_at: u64::read(buf)?,
        })
    }
}

impl Write for Registry {
    fn write(&self, buf: &mut impl BufMut) {
        self.body.write(buf);
        self.signature.write(buf);
    }
}

impl EncodeSize for Registry {
    fn encode_size(&self) -> usize {
        self.body.encode_size() + self.signature.encode_size()
    }
}

impl Read for Registry {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            body: RegistryBody::read(buf)?,
            signature: Signature::read(buf)?,
        })
    }
}
