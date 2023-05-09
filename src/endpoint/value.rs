use core::convert::TryFrom;
use core::fmt::Debug;
use core::ops::Deref;
use core::{fmt::Display, str::FromStr};

pub trait BytesIsh = AsRef<[u8]> + Debug;
pub trait StringIsh = AsRef<str> + Debug;

use heapless::{String, Vec};

use crate::prelude::IotError;

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

impl Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Value::Text(v) => Display::fmt(v, f),
            Value::Int32(v) => Display::fmt(v, f),
            Value::Float32(v) => match f.width() {
                Some(w) => write!(f, "{v:w$.02}"),
                None => write!(f, "{v:.02}"),
            },
            Value::Bool(v) => Display::fmt(v, f),
            Value::Bytes(v) => write!(f, "{v:02x?}"),
        }
    }
}

impl FromStr for Value {
    type Err = IotError;

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
pub(crate) fn parse_endpoint_value(src: &str) -> Result<Value, IotError> {
    Value::from_str(src)
}
