use core::str::FromStr;
use core::fmt::Debug;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use byteorder::{ByteOrder, NetworkEndian};

use super::{BytesIsh, StringIsh};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Value<S: StringIsh= String, B: BytesIsh = Vec<u8>> {
    /// Boolean value
    Bool(bool),
    /// 32-bit integer value
    Int32(i32),
    /// 32-bit floating point value
    Float32(f32),
    /// String value
    Text(S),
    /// Raw data value
    Bytes(B),
}

pub type ValueRef<'a> = Value<&'a str, &'a [u8]>;

pub type ValueOwned = Value<String, Vec<u8>>;

impl <S: AsRef<str> + Debug, B: BytesIsh> From<bool> for Value<S, B> {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl <S: AsRef<str> + Debug, B: BytesIsh> From<i32> for Value<S, B> {
    fn from(v: i32) -> Self {
        Self::Int32(v)
    }
}

impl <S: AsRef<str> + Debug, B: BytesIsh> From<f32> for Value<S, B> {
    fn from(v: f32) -> Self {
        Self::Float32(v)
    }
}

impl <B: BytesIsh> From<String> for Value<String, B> {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl <S: AsRef<str> + Debug> From<Vec<u8>> for Value<S, Vec<u8>> {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}


impl <S: AsRef<str> + Debug, B: BytesIsh> core::fmt::Display for Value<S, B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Value::Text(v) => write!(f, "{}", v.as_ref()),
            Value::Int32(v) => write!(f, "{}", v),
            Value::Float32(v) => write!(f, "{:.02}", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Bytes(v) => write!(f, "{:02x?}", v),
        }
    }
}


impl FromStr for Value<String, Vec<u8>> {
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
