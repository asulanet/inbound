use crate::{Error, MAX_NAME_BYTES};
use bytes::{Buf, BufMut};
use commonware_codec::{EncodeSize, Read, Write};

macro_rules! name_type {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            pub fn new(name: impl Into<String>) -> Result<Self, Error> {
                let name = name.into();
                if name.is_empty() || name.len() > MAX_NAME_BYTES {
                    return Err(Error::Decode(format!("{} length {}", $label, name.len())));
                }
                Ok(Self(name))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl Write for $name {
            fn write(&self, buf: &mut impl BufMut) {
                self.0.as_bytes().to_vec().write(buf);
            }
        }

        impl EncodeSize for $name {
            fn encode_size(&self) -> usize {
                self.0.as_bytes().to_vec().encode_size()
            }
        }

        impl Read for $name {
            type Cfg = ();

            fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
                let raw = Vec::<u8>::read_cfg(buf, &((1..=MAX_NAME_BYTES).into(), ()))?;
                let name = String::from_utf8(raw)
                    .map_err(|_| commonware_codec::Error::Invalid($label, "not utf-8"))?;
                Ok(Self(name))
            }
        }
    };
}

name_type!(ChannelId, "ChannelId");
name_type!(FormatId, "FormatId");

pub(crate) fn read_tag(buf: &mut impl Buf) -> Result<u8, commonware_codec::Error> {
    u8::read_cfg(buf, &())
}

pub(crate) fn unknown_tag(tag: u8) -> commonware_codec::Error {
    commonware_codec::Error::InvalidEnum(tag)
}
