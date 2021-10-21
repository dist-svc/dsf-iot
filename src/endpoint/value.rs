use core::str::FromStr;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Value {
    /// Boolean value
    Bool(bool),
    /// 32-bit integer value
    Int32(i32),
    /// 32-bit floating point value
    Float32(f32),
    /// String value
    Text(String),
    /// Raw data value
    Bytes(Vec<u8>),
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int32(v)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Self::Float32(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}


impl core::fmt::Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Value::Text(v) => write!(f, "{}", v),
            Value::Int32(v) => write!(f, "{}", v),
            Value::Float32(v) => write!(f, "{:.02}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Bytes(v) => write!(f, "{:02x?}", v),
        }
    }
}

impl FromStr for Value {
    type Err = String;

    fn from_str(src: &str) -> Result<Value, Self::Err> {
        // first attempt to match bools
        if src.to_lowercase() == "true" {
            return Ok(Value::Bool(true));
        } else if src.to_lowercase() == "false" {
            return Ok(Value::Bool(false));
        }

        // Then floats
        if let Ok(v) = f32::from_str(src) {
            return Ok(Value::Float32(v));
        }

        // TODO: then bytes

        // Otherwise it's probably a string
        Ok(Value::Text(src.to_string()))
    }
}

/// Helper to parse endpoint data from string values
pub(crate) fn parse_endpoint_value(src: &str) -> Result<Value, String> {
    Value::from_str(src)
}
