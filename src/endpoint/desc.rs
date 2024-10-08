use core::convert::TryFrom;
use core::fmt::Debug;
use core::ops::Deref;

use dsf_core::error::Error;

use log::{error, trace, warn};

use byteorder::{ByteOrder, LittleEndian};
use heapless::String;

//use modular_bitfield::prelude::*;

use crate::prelude::IotError;

use super::kinds::*;
use super::value::*;

/// IoT Option IDs, used for identifying descriptors and data.
pub mod iot_option_kinds {
    pub const ENDPOINT_DESCRIPTOR: u16 = 0x0001 | (1 << 15);
    pub const VALUE_BOOL_FALSE: u16 = 0x0002 | (1 << 15);
    pub const VALUE_BOOL_TRUE: u16 = 0x0003 | (1 << 15);
    pub const VALUE_FLOAT: u16 = 0x0004 | (1 << 15);
    pub const VALUE_INT: u16 = 0x0005 | (1 << 15);
    pub const VALUE_STRING: u16 = 0x0006 | (1 << 15);
    pub const VALUE_RAW: u16 = 0x0007 | (1 << 15);
    pub const VALUE_RGB: u16 = 0x0008 | (1 << 15);

    pub const ENDPOINT_DESCRIPTOR_LEN: usize = 4;
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct EpFlags: u16 {
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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub struct EpDescriptor {
    /// Endpoint Kind
    pub kind: EpKind,

    /// Endpoint flags
    pub flags: EpFlags,
}

impl EpDescriptor {
    pub fn new(kind: EpKind, flags: EpFlags) -> Self {
        Self { kind, flags }
    }
}

impl core::fmt::Display for EpDescriptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:16} in {:4}\r\n", self.kind, self.kind.unit())
    }
}

impl encdec::Encode for EpDescriptor {
    type Error = Error;

    fn encode_len(&self) -> Result<usize, Self::Error> {
        Ok(8)
    }

    fn encode(&self, data: &mut [u8]) -> Result<usize, Error> {
        // Write option header (option kind and length)
        LittleEndian::write_u16(&mut data[0..], iot_option_kinds::ENDPOINT_DESCRIPTOR);
        LittleEndian::write_u16(
            &mut data[2..],
            iot_option_kinds::ENDPOINT_DESCRIPTOR_LEN as u16,
        );

        // Write option data (endpoint kind, reserved flags)
        LittleEndian::write_u16(&mut data[4..], u16::from(&self.kind));
        LittleEndian::write_u16(&mut data[6..], self.flags.bits());

        // TODO: write metadata

        Ok(8)
    }
}

impl encdec::DecodeOwned for EpDescriptor {
    type Error = Error;
    type Output = EpDescriptor;

    fn decode_owned(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        trace!("Parsing: {:x?}", buff);

        // Read option header (kind and length)
        let option_kind = LittleEndian::read_u16(buff);

        if option_kind != iot_option_kinds::ENDPOINT_DESCRIPTOR {
            warn!("Unrecognised option kind: {}", option_kind);
            return Err(Error::InvalidOption);
        }
        let len = LittleEndian::read_u16(&buff[2..]) + 4;

        // Parse out endpoint index and kind
        let kind = LittleEndian::read_u16(&buff[4..]).into();
        let flags = LittleEndian::read_u16(&buff[6..]);
        let flags = EpFlags::from_bits_truncate(flags);

        // TODO: read metadata

        Ok((Self { kind, flags }, len as usize))
    }
}

pub fn parse_endpoint_descriptor(src: &str) -> Result<EpDescriptor, IotError> {
    let kind = parse_endpoint_kind(src)?;
    Ok(EpDescriptor::new(kind, EpFlags::empty()))
}

/// Endpoint data object contains data associated with a specific endpoint
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EpData {
    // Measurement value
    pub value: EpValue,
}

impl EpData {
    pub fn new(value: EpValue) -> Self {
        Self { value }
    }
}

impl encdec::DecodeOwned for EpData {
    type Error = Error;
    type Output = EpData;

    fn decode_owned(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        use iot_option_kinds::*;

        // Read option header (kind and length)
        let kind = LittleEndian::read_u16(&buff[0..]);
        let len = LittleEndian::read_u16(&buff[2..]);

        let value = match kind {
            VALUE_BOOL_FALSE => EpValue::Bool(false),
            VALUE_BOOL_TRUE => EpValue::Bool(true),
            VALUE_FLOAT => {
                let f = LittleEndian::read_f32(&buff[4..]);
                EpValue::Float32(f)
            }
            VALUE_INT => {
                let f = LittleEndian::read_i32(&buff[4..]);
                EpValue::Int32(f)
            }
            VALUE_STRING => {
                let s = core::str::from_utf8(&buff[4..]).unwrap();
                EpValue::Text(String::from(s))
            }
            VALUE_RAW => {
                let s = &buff[4..][..len as usize];
                EpValue::try_from(s).map_err(|_| Error::InvalidOption)?
            }
            VALUE_RGB => {
                let u = LittleEndian::read_u32(&buff[4..]);
                EpValue::Rgb((u >> 16) as u8, (u >> 8) as u8, u as u8)
            }
            _ => {
                error!("Unrecognised option kind: 0x{:x?}", kind);
                return Err(Error::InvalidOption);
            }
        };

        // TODO: read metadata

        Ok((Self { value }, len as usize + 4))
    }
}

impl encdec::Encode for EpData {
    type Error = Error;

    fn encode_len(&self) -> Result<usize, Self::Error> {
        let n = match &self.value {
            EpValue::Bool(_) => 4,
            EpValue::Float32(_) | EpValue::Int32(_) => 8,
            EpValue::Text(v) => {
                let b = v.deref().as_bytes();
                4 + b.len()
            }
            EpValue::Bytes(v) => 4 + v.len(),
            EpValue::Rgb(_, _, _) => 8,
        };

        Ok(n)
    }

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        use iot_option_kinds::*;

        // Write option header and data
        let len = match &self.value {
            EpValue::Bool(v) if *v == true => {
                LittleEndian::write_u16(&mut buff[0..], VALUE_BOOL_TRUE);
                LittleEndian::write_u16(&mut buff[2..], 0);
                4
            }
            EpValue::Bool(v) if *v == false => {
                LittleEndian::write_u16(&mut buff[0..], VALUE_BOOL_FALSE);
                LittleEndian::write_u16(&mut buff[2..], 0);
                4
            }
            EpValue::Float32(v) => {
                LittleEndian::write_u16(&mut buff[0..], VALUE_FLOAT);
                LittleEndian::write_u16(&mut buff[2..], 4);
                LittleEndian::write_f32(&mut buff[4..], *v);
                8
            }
            EpValue::Int32(v) => {
                LittleEndian::write_u16(&mut buff[0..], VALUE_INT);
                LittleEndian::write_u16(&mut buff[2..], 4);
                LittleEndian::write_i32(&mut buff[4..], *v);
                8
            }
            EpValue::Text(v) => {
                let b = v.deref().as_bytes();

                LittleEndian::write_u16(&mut buff[0..], VALUE_STRING);
                LittleEndian::write_u16(&mut buff[2..], b.len() as u16);
                (&mut buff[4..4 + b.len()]).copy_from_slice(b);
                4 + b.len()
            }
            EpValue::Bytes(v) => {
                LittleEndian::write_u16(&mut buff[0..], VALUE_RAW);
                LittleEndian::write_u16(&mut buff[2..], v.len() as u16);
                (&mut buff[4..4 + v.len()]).copy_from_slice(&v);
                4 + v.len()
            }
            EpValue::Rgb(r, g, b) => {
                let u = (*r as u32) << 16 | (*g as u32) << 8 | (*b as u32);

                LittleEndian::write_u16(&mut buff[0..], VALUE_RGB);
                LittleEndian::write_u16(&mut buff[2..], 4);
                LittleEndian::write_u32(&mut buff[4..], u);
                8
            }
            _ => unimplemented!("Encode not yet implemented for value: {:?}", self),
        };

        // TODO: write metadata

        Ok(len)
    }
}

pub fn parse_endpoint_data(src: &str) -> Result<EpData, IotError> {
    let value = parse_endpoint_value(src)?;
    Ok(EpData::new(value))
}

#[cfg(test)]
mod tests {
    use encdec::{Decode, Encode};

    use super::*;

    #[test]
    fn encode_decode_endpoint_descriptor() {
        let descriptors = vec![
            EpDescriptor {
                kind: EpKind::Temperature,
                flags: EpFlags::R,
            },
            EpDescriptor {
                kind: EpKind::Pressure,
                flags: EpFlags::W,
            },
            EpDescriptor {
                kind: EpKind::Humidity,
                flags: EpFlags::RW,
            },
        ];

        for descriptor in &descriptors {
            let mut buff = vec![0u8; 1024];

            let n = descriptor.encode(&mut buff).expect("Encoding error");

            trace!("Encoded {:?} to: {:0x?}", descriptor, &buff[..n]);

            let (d, _n) = EpDescriptor::decode(&buff[..n]).expect("Decoding error");

            assert_eq!(descriptor, &d);
        }
    }

    #[test]
    fn encode_decode_endpoint_data() {
        let data = vec![
            EpData {
                value: EpValue::Bool(true),
            },
            EpData {
                value: EpValue::Bool(false),
            },
            EpData {
                value: EpValue::Float32(10.45),
            },
            EpData {
                value: EpValue::Rgb(10, 20, 30),
            }
        ];

        for d in &data {
            let mut buff = vec![0u8; 1024];

            let n = d.encode(&mut buff).expect("Encoding error");

            trace!("Encoded {:?} to: {:0x?}", d, &buff[..n]);

            let (d1, _n) = EpData::decode(&buff[..n]).expect("Decoding error");

            assert_eq!(d, &d1);
        }
    }
}
