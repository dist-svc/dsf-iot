
use std::str::FromStr;
use std::io::Cursor;

use serde::{Serialize, Deserialize};

use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

use dsf_core::base::{Parse, Encode};
use dsf_core::options::{OptionsError, Metadata};

pub mod kinds;
pub use kinds::*;

pub mod value;
pub use value::*;

pub enum Options {
    Descriptor = 1,
    ValueBool = 2,
    ValueFloat = 3,
    ValueString = 4,
}

pub const ENDPOINT_DESCRIPTOR_LEN: usize = 4;

/// An endpoint descriptor defines the kind of an endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointDescriptor {
    /// Endpoint index
    pub index: u16,

    /// Endpoint Data Kind
    pub kind: EndpointKind,

    /// Endpoint metadata
    pub meta: Vec<Metadata>,
}

/// Endpoint data object contains data associated with a specific endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointData {
    /// Endpoint index
    pub index: u16,

    // Measurement value
    pub value: EndpointValue,

    /// Measurement metadata
    pub meta: Vec<Metadata>,
}


impl EndpointDescriptor {

    fn parse(data: &[u8]) -> Result<(Self, usize), OptionsError> {
        unimplemented!()
    }

    fn encode(&self, data: &mut [u8]) -> Result<usize, OptionsError> {
        let mut w = Cursor::new(data);
        
        w.write_u16::<NetworkEndian>(Options::Descriptor as u16)?;
        w.write_u16::<NetworkEndian>(u16::from(&self.kind))?;

        
        w.write(&self.public_key)?;

        Ok(w.position() as usize)
    }
}



#[cfg(test)]
mod tests {

    #[test]
    fn encode_decode_endpoint_descriptor() {

    }

}