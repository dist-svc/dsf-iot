use std::str::FromStr;

use serde::{Deserialize, Serialize};

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

impl From<bool> for EndpointValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<f32> for EndpointValue {
    fn from(v: f32) -> Self {
        Self::Float32(v)
    }
}

impl From<String> for EndpointValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<Vec<u8>> for EndpointValue {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

impl std::fmt::Display for EndpointValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointValue::Text(v) => write!(f, "{}", v),
            EndpointValue::Float32(v) => write!(f, "{:.02}", v),
            EndpointValue::Bool(v) => write!(f, "{}", v),
            EndpointValue::Bytes(v) => write!(f, "{:02x?}", v),
        }
    }
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
