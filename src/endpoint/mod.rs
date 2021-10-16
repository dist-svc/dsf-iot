#[cfg(feature = "alloc")]
use alloc::prelude::v1::*;

use log::{error, trace, warn};

use byteorder::{ByteOrder, NetworkEndian};

use dsf_core::error::Error;
use dsf_core::options::Metadata;

pub mod kinds;
pub use kinds::*;

pub mod value;
pub use value::*;

pub mod meta;
pub use meta::*;

/// IoT Option IDs, used for identifying descriptors and data.
pub mod iot_option_kinds {
    pub const ENDPOINT_DESCRIPTOR: u16  = 0x0001 | (1 << 15);
    pub const VALUE_BOOL_FALSE: u16     = 0x0002 | (1 << 15);
    pub const VALUE_BOOL_TRUE: u16      = 0x0003 | (1 << 15);
    pub const VALUE_FLOAT: u16          = 0x0004 | (1 << 15); 
    pub const VALUE_STRING: u16         = 0x0005 | (1 << 15);

    pub const ENDPOINT_DESCRIPTOR_LEN: usize = 4;
}


bitflags::bitflags! {
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    pub struct Flags: u16 {
        /// Read flag
        const R = 0b0000_0001;
        /// Write flag
        const W = 0b0000_0010;

        /// Combined read/write
        const RW = Self::R.bits() | Self::W.bits();
    }
}


/// An endpoint descriptor defines the kind of an endpoint
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Descriptor {
    /// Endpoint Kind
    pub kind: Kind,

    /// Endpoint flags
    pub flags: Flags,

    /// Endpoint metadata
    pub meta: Vec<Metadata>,
}

/// Endpoint data object contains data associated with a specific endpoint
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Data {
    // Measurement value
    pub value: Value,

    /// Measurement metadata
    pub meta: Vec<Metadata>,
}

impl Descriptor {
    pub fn new(kind: Kind, flags: Flags, meta: &[Metadata]) -> Self {
        Self {
            kind,
            flags,
            meta: meta.to_vec(),
        }
    }

    pub fn parse(data: &[u8]) -> Result<(Self, usize), Error> {
        trace!("Parsing: {:x?}", data);

        // Read option header (kind and length)
        let option_kind = NetworkEndian::read_u16(data);

        if option_kind != iot_option_kinds::ENDPOINT_DESCRIPTOR {
            warn!("Unrecognised option kind: {}", option_kind);
            return Err(Error::InvalidOption);
        }
        let len = NetworkEndian::read_u16(&data[2..]) + 4;

        // Parse out endpoint index and kind
        let kind = NetworkEndian::read_u16(&data[4..]).into();
        let flags = NetworkEndian::read_u16(&data[6..]);
        let flags = Flags::from_bits_truncate(flags);

        // TODO: read metadata

        Ok((
            Self {
                kind,
                flags,
                meta: Vec::new(),
            },
            len as usize,
        ))
    }
}

impl dsf_core::base::Encode for Descriptor {
    type Error = Error;

    fn encode(&self, data: &mut [u8]) -> Result<usize, Error> {
        // Write option header (option kind and length)
        NetworkEndian::write_u16(&mut data[0..], iot_option_kinds::ENDPOINT_DESCRIPTOR);
        NetworkEndian::write_u16(
            &mut data[2..],
            iot_option_kinds::ENDPOINT_DESCRIPTOR_LEN as u16,
        );

        // Write option data (endpoint kind, reserved flags)
        NetworkEndian::write_u16(&mut data[4..], u16::from(&self.kind));
        NetworkEndian::write_u16(&mut data[6..], self.flags.bits());

        // TODO: write metadata

        Ok(8)
    }
}

#[cfg(feature = "std")]
pub fn parse_endpoint_descriptor(src: &str) -> Result<Descriptor, String> {
    let kind = parse_endpoint_kind(src)?;
    Ok(Descriptor::new(kind, Flags::empty(), &[]))
}

impl Data {
    pub fn new(value: Value, meta: &[Metadata]) -> Self {
        Self {
            value,
            meta: meta.to_vec(),
        }
    }

    pub fn parse(data: &[u8]) -> Result<(Self, usize), Error> {
        use iot_option_kinds::*;

        // Read option header (kind and length)
        let kind = NetworkEndian::read_u16(&data[0..]);
        let len = NetworkEndian::read_u16(&data[2..]);

        let value = match kind {
            VALUE_BOOL_FALSE => Value::Bool(false),
            VALUE_BOOL_TRUE => Value::Bool(true),
            VALUE_FLOAT => {
                let f = NetworkEndian::read_f32(&data[4..]);
                Value::Float32(f)
            }
            VALUE_STRING => {
                let s = core::str::from_utf8(&data[4..]).unwrap();
                Value::Text(s.to_owned())
            }
            _ => {
                error!("Unrecognised option kind: 0x{:x?}", kind);
                return Err(Error::InvalidOption);
            }
        };

        // TODO: read metadata

        Ok((
            Self {
                value,
                meta: Vec::new(),
            },
            len as usize + 4,
        ))
    }

    pub fn encode(&self, data: &mut [u8]) -> Result<usize, Error> {
        use iot_option_kinds::*;

        // Write option header and data
        let len = match &self.value {
            Value::Bool(v) if *v == true => {
                NetworkEndian::write_u16(&mut data[0..], VALUE_BOOL_TRUE);
                NetworkEndian::write_u16(&mut data[2..], 0);
                4
            }
            Value::Bool(v) if *v == false => {
                NetworkEndian::write_u16(&mut data[0..], VALUE_BOOL_FALSE);
                NetworkEndian::write_u16(&mut data[2..], 0);
                4
            }
            Value::Float32(v) => {
                NetworkEndian::write_u16(&mut data[0..], VALUE_FLOAT);
                NetworkEndian::write_u16(&mut data[2..], 4);
                NetworkEndian::write_f32(&mut data[4..], *v);
                8
            }
            Value::Text(v) => {
                let b = v.as_bytes();

                NetworkEndian::write_u16(&mut data[0..], VALUE_STRING);
                NetworkEndian::write_u16(&mut data[2..], b.len() as u16);
                (&mut data[4..4 + b.len()]).copy_from_slice(b);
                4 + b.len()
            }
            _ => unimplemented!(),
        };

        // TODO: write metadata

        Ok(len)
    }
}

pub fn parse_endpoint_data(src: &str) -> Result<Data, String> {
    let value = parse_endpoint_value(src)?;
    Ok(Data::new(value, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    use dsf_core::base::Encode;

    #[test]
    fn encode_decode_endpoint_descriptor() {
        let descriptors = vec![
            Descriptor {
                kind: Kind::Temperature,
                flags: Flags::R,
                meta: vec![],
            },
            Descriptor {
                kind: Kind::Pressure,
                flags: Flags::W,
                meta: vec![],
            },
            Descriptor {
                kind: Kind::Humidity,
                flags: Flags::RW,
                meta: vec![],
            },
        ];

        for descriptor in &descriptors {
            let mut buff = vec![0u8; 1024];

            let n = descriptor.encode(&mut buff).expect("Encoding error");

            trace!("Encoded {:?} to: {:0x?}", descriptor, &buff[..n]);

            let (d, _n) = Descriptor::parse(&buff[..n]).expect("Decoding error");

            assert_eq!(descriptor, &d);
        }
    }

    #[test]
    fn encode_decode_endpoint_data() {
        let data = vec![
            Data {
                value: Value::Bool(true),
                meta: vec![],
            },
            Data {
                value: Value::Bool(false),
                meta: vec![],
            },
            Data {
                value: Value::Float32(10.45),
                meta: vec![],
            },
        ];

        for d in &data {
            let mut buff = vec![0u8; 1024];

            let n = d.encode(&mut buff).expect("Encoding error");

            trace!("Encoded {:?} to: {:0x?}", d, &buff[..n]);

            let (d1, _n) = Data::parse(&buff[..n]).expect("Decoding error");

            assert_eq!(d, &d1);
        }
    }
}
