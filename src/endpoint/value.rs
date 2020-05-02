
use std::str::FromStr;

use serde::{Serialize, Deserialize};

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

impl FromStr for EndpointValue {
    type Err = String;

    fn from_str(src: &str) -> Result<EndpointValue, Self::Err> {
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

        // Otherwise it's probably a string
        Ok(EndpointValue::Text(src.to_string()))
    }
}

/// Helper to parse endpoint data from string values
pub(crate) fn parse_endpoint_value(src: &str) -> Result<EndpointValue, String> {
    EndpointValue::from_str(src)   
}

