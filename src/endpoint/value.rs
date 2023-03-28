use core::str::FromStr;
use core::fmt::Debug;
use core::ops::Deref;
use core::convert::TryFrom;

pub trait BytesIsh = AsRef<[u8]> + Debug;
pub trait StringIsh = AsRef<str> + Debug;

use heapless::{String, Vec};


#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum Value {
    /// Boolean value
    Bool(bool),
    /// 32-bit integer value
    Int32(i32),
    /// 32-bit floating point value
    Float32(f32),
    /// String value
    Text(String<64>),
    /// Raw data value
    Bytes(Vec<u8, 64>),
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

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(String::from(v))
    }
}


impl TryFrom<&[u8]> for Value {
    type Error = ();

    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self::Bytes(Vec::try_from(v)?))
    }
}


impl core::fmt::Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Value::Text(v) => write!(f, "{}", v.deref()),
            Value::Int32(v) => write!(f, "{}", v),
            Value::Float32(v) => write!(f, "{:.02}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Bytes(v) => write!(f, "{:02x?}", v),
        }
    }
}

impl FromStr for Value {
    type Err = &'static str;

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
        Ok(Value::from(src))
    }
}

/// Helper to parse endpoint data from string values
pub(crate) fn parse_endpoint_value(src: &str) -> Result<Value, &'static str> {
    Value::from_str(src)
}
