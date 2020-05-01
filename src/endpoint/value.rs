
use std::str::FromStr;

use byteorder::NetworkEndian;
use serde::{Serialize, Deserialize};

use dsf_core::base::{Parse, Encode};
use dsf_core::options::OptionsError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EndpointValue {
    /// Boolean value
    Bool(bool),
    /// 32-bit floating point value
    Float32(f32),
    /// String value
    Text(String),
    /// Raw data value
    Bytes(Vec<u8>),
}

pub fn parse_endpoint_data(src: &str) -> Result<EndpointValue, String> {

    // first attempt to match bools
    if src.to_lowercase() == "true" {
        return Ok(EndpointValue::Bool(true));
    } else if src == "false" {
        return Ok(EndpointValue::Bool(false));
    }

    // Then floats
    if let Ok(v) = f32::from_str(src) {
        return Ok(EndpointValue::Float32(v));
    }

    // TODO: then bytes

    // Then it's probably a string
    Ok(EndpointValue::Text(src.to_string()))
}

