use core::{
    convert::TryFrom,
    fmt::{Debug, Display},
    str::FromStr,
};

use heapless::{String, Vec};

use crate::prelude::IotError;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum EpValue {
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

impl From<bool> for EpValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for EpValue {
    fn from(v: i32) -> Self {
        Self::Int32(v)
    }
}

impl From<f32> for EpValue {
    fn from(v: f32) -> Self {
        Self::Float32(v)
    }
}

impl From<&str> for EpValue {
    fn from(v: &str) -> Self {
        Self::Text(String::from(v))
    }
}

impl TryFrom<&[u8]> for EpValue {
    type Error = ();

    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self::Bytes(Vec::try_from(v)?))
    }
}

impl <const N: usize> TryFrom<&[u8; N]> for EpValue {
    type Error = ();

    fn try_from(v: &[u8; N]) -> Result<Self, Self::Error> {
        Ok(Self::Bytes(Vec::try_from(v.as_slice())?))
    }
}

impl Display for EpValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EpValue::Text(v) => Display::fmt(v, f),
            EpValue::Int32(v) => Display::fmt(v, f),
            EpValue::Float32(v) => match f.width() {
                Some(w) => write!(f, "{v:w$.02}"),
                None => write!(f, "{v:.02}"),
            },
            EpValue::Bool(v) => Display::fmt(v, f),
            EpValue::Bytes(v) => write!(f, "{v:02x?}"),
        }
    }
}

impl FromStr for EpValue {
    type Err = IotError;

    fn from_str(src: &str) -> Result<EpValue, Self::Err> {
        // first attempt to match bools
        if src.to_lowercase() == "true" {
            return Ok(EpValue::Bool(true));
        } else if src.to_lowercase() == "false" {
            return Ok(EpValue::Bool(false));
        }

        // Then floats
        if let Ok(v) = f32::from_str(src) {
            return Ok(EpValue::Float32(v));
        }

        // TODO: then bytes

        // Otherwise it's probably a string
        Ok(EpValue::from(src))
    }
}

/// Helper to parse endpoint data from string values
pub(crate) fn parse_endpoint_value(src: &str) -> Result<EpValue, IotError> {
    EpValue::from_str(src)
}
