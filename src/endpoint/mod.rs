use std::io::{Cursor, Write};

use log::{trace, debug, info};

use serde::{Deserialize, Serialize};

use byteorder::{ByteOrder, NetworkEndian, WriteBytesExt};

use dsf_core::options::{Metadata, OptionsError};

pub mod kinds;
pub use kinds::*;

pub mod value;
pub use value::*;

pub mod iot_option_kinds {
    pub const ENDPOINT_DESCRIPTOR: u16 = 0x0001 | (1 << 15);
    pub const VALUE_BOOL_FALSE: u16 = 0x0002 | (1 << 15);
    pub const VALUE_BOOL_TRUE: u16 = 0x0003 | (1 << 15);
    pub const VALUE_FLOAT: u16 = 0x0004 | (1 << 15);
    pub const VALUE_STRING: u16 = 0x0005 | (1 << 15);

    pub const ENDPOINT_DESCRIPTOR_LEN: usize = 4;
}

/// An endpoint descriptor defines the kind of an endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointDescriptor {
    /// Endpoint Data Kind
    pub kind: EndpointKind,

    /// Endpoint metadata
    pub meta: Vec<Metadata>,
}

/// Endpoint data object contains data associated with a specific endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointData {
    // Measurement value
    pub value: EndpointValue,

    /// Measurement metadata
    pub meta: Vec<Metadata>,
}

impl EndpointDescriptor {
    pub fn new(kind: EndpointKind, meta: &[Metadata]) -> Self {
        Self {
            kind,
            meta: meta.to_vec(),
        }
    }

    pub fn parse(data: &[u8]) -> Result<(Self, usize), OptionsError> {
        trace!("Parsing: {:x?}", data);

        // Read option header (kind and length)
        let option_kind = NetworkEndian::read_u16(data);

        if option_kind != iot_option_kinds::ENDPOINT_DESCRIPTOR {
            warn!("Unrecognised option kind: {}", option_kind);
            return Err(OptionsError::InvalidOptionKind);
        }
        let len = NetworkEndian::read_u16(&data[2..]) + 4;

        // Parse out endpoint index and kind
        let kind = NetworkEndian::read_u16(&data[4..]).into();
        let _flags = NetworkEndian::read_u16(&data[6..]);

        // TODO: read metadata

        Ok((Self { kind, meta: vec![] }, len as usize))
    }

    pub fn encode(&self, data: &mut [u8]) -> Result<usize, OptionsError> {
        let mut w = Cursor::new(data);

        // Write option header (option kind and length)
        w.write_u16::<NetworkEndian>(iot_option_kinds::ENDPOINT_DESCRIPTOR)?;
        w.write_u16::<NetworkEndian>(iot_option_kinds::ENDPOINT_DESCRIPTOR_LEN as u16)?;

        // Write option data (endpoint kind, reserved flags)
        w.write_u16::<NetworkEndian>(u16::from(&self.kind))?;
        w.write_u16::<NetworkEndian>(0)?;

        // TODO: write metadata

        Ok(w.position() as usize)
    }
}

pub fn parse_endpoint_descriptor(src: &str) -> Result<EndpointDescriptor, String> {
    let kind = parse_endpoint_kind(src)?;
    Ok(EndpointDescriptor::new(kind, &[]))
}

impl EndpointData {
    pub fn new(value: EndpointValue, meta: &[Metadata]) -> Self {
        Self {
            value,
            meta: meta.to_vec(),
        }
    }

    pub fn parse(data: &[u8]) -> Result<(Self, usize), OptionsError> {
        use iot_option_kinds::*;

        info!("Decoding: {:x?}", data);

        // Read option header (kind and length)
        let kind = NetworkEndian::read_u16(&data[0..]);
        let len = NetworkEndian::read_u16(&data[2..]);

        let value = match kind {
            VALUE_BOOL_FALSE => EndpointValue::Bool(false),
            VALUE_BOOL_TRUE => EndpointValue::Bool(true),
            VALUE_FLOAT => {
                let f = NetworkEndian::read_f32(&data[4..]);
                EndpointValue::Float32(f)
            }
            VALUE_STRING => {
                let s = std::str::from_utf8(&data[4..]).unwrap();
                EndpointValue::Text(s.to_owned())
            }
            _ => {
                error!("Unrecognised option kind: 0x{:x?}", kind);
                return Err(OptionsError::InvalidOptionKind);
            }
        };

        // TODO: read metadata

        Ok((
            Self {
                value,
                meta: vec![],
            },
            len as usize + 4,
        ))
    }

    pub fn encode(&self, data: &mut [u8]) -> Result<usize, OptionsError> {
        use iot_option_kinds::*;

        let mut w = Cursor::new(data);

        // Write option header and data
        match &self.value {
            EndpointValue::Bool(v) if *v == true => {
                w.write_u16::<NetworkEndian>(VALUE_BOOL_TRUE)?;
                w.write_u16::<NetworkEndian>(0)?;
            }
            EndpointValue::Bool(v) if *v == false => {
                w.write_u16::<NetworkEndian>(VALUE_BOOL_FALSE)?;
                w.write_u16::<NetworkEndian>(0)?;
            }
            EndpointValue::Float32(v) => {
                w.write_u16::<NetworkEndian>(VALUE_FLOAT)?;
                w.write_u16::<NetworkEndian>(4)?;
                w.write_f32::<NetworkEndian>(*v)?;
            }
            EndpointValue::Text(v) => {
                let b = v.as_bytes();

                w.write_u16::<NetworkEndian>(VALUE_STRING)?;
                w.write_u16::<NetworkEndian>(b.len() as u16)?;
                w.write(b)?;
            }
            _ => unimplemented!(),
        }

        // TODO: write metadata

        Ok(w.position() as usize)
    }
}

pub fn parse_endpoint_data(src: &str) -> Result<EndpointData, String> {
    let value = parse_endpoint_value(src)?;
    Ok(EndpointData::new(value, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_endpoint_descriptor() {
        let descriptors = vec![
            EndpointDescriptor {
                kind: EndpointKind::Temperature,
                meta: vec![],
            },
            EndpointDescriptor {
                kind: EndpointKind::Pressure,
                meta: vec![],
            },
            EndpointDescriptor {
                kind: EndpointKind::Humidity,
                meta: vec![],
            },
        ];

        for descriptor in &descriptors {
            let mut buff = vec![0u8; 1024];

            let n = descriptor.encode(&mut buff).expect("Encoding error");

            println!("Encoded {:?} to: {:0x?}", descriptor, &buff[..n]);

            let (d, _n) = EndpointDescriptor::parse(&buff[..n]).expect("Decoding error");

            assert_eq!(descriptor, &d);
        }
    }

    #[test]
    fn encode_decode_endpoint_data() {
        let data = vec![
            EndpointData {
                value: EndpointValue::Bool(true),
                meta: vec![],
            },
            EndpointData {
                value: EndpointValue::Bool(false),
                meta: vec![],
            },
            EndpointData {
                value: EndpointValue::Float32(10.45),
                meta: vec![],
            },
        ];

        for d in &data {
            let mut buff = vec![0u8; 1024];

            let n = d.encode(&mut buff).expect("Encoding error");

            println!("Encoded {:?} to: {:0x?}", d, &buff[..n]);

            let (d1, _n) = EndpointData::parse(&buff[..n]).expect("Decoding error");

            assert_eq!(d, &d1);
        }
    }
}
