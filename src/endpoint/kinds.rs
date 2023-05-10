use core::fmt::Write;
use core::str::FromStr;

#[cfg(feature = "alloc")]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::prelude::IotError;

/// Available endpoint descriptors, their names, units, and IDs
pub const ENDPOINT_KINDS: &[(u16, EpKind, &str, &str)] = &[
    (1, EpKind::Temperature, "temperature", "°C"),
    (2, EpKind::Humidity, "humidity", "%RH"),
    (3, EpKind::Pressure, "pressure", "kPa"),
    (4, EpKind::Co2, "CO2", "ppm"),
    (5, EpKind::State, "state", "bool"),
    (6, EpKind::Brightness, "brightness", "%"),
    (7, EpKind::Colour, "colour", "rgb"),
];

/// [`Kind`] specifies the type of IoT endpoint, translated using the [`ENDPOINT_KINDS`] table
/// For example: Temperature, Heart-Rate
#[derive(Debug, Copy, Clone, PartialEq, strum::Display)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum EpKind {
    /// Temperature in (degrees Celcius)
    Temperature,
    /// Humidity (in % RH)
    Humidity,
    /// Pressure (in kPa)
    Pressure,
    /// CO2 content in PPM
    Co2,
    /// State (on/off)
    State,
    /// Brightness as a percentage
    Brightness,
    /// RGB encoded colour
    Colour,
    /// Unknown measurement kind (no units)
    Unknown(u16),
}

/// Parse an endpoint kind from a string
pub fn parse_endpoint_kind(src: &str) -> Result<EpKind, IotError> {
    // Coerce to lower case
    let src = src.to_lowercase();

    // Attempt to find matching endpoint name
    if let Ok(v) = EpKind::from_str(&src) {
        return Ok(v);
    }

    // Attempt to parse as an integer
    if let Ok(v) = u16::from_str(&src) {
        return Ok(EpKind::Unknown(v));
    }

    Err(IotError::UnrecognisedEndpoint)
}

impl EpKind {
    /// List available endpoint variants
    pub fn variants() -> String {
        let mut buff = String::new();

        for (i, _k, s, u) in ENDPOINT_KINDS {
            write!(&mut buff, "'{}' (unit: {}, id: {}), ", s, u, i).unwrap();
        }

        write!(&mut buff, "RAW_ID (no unit)").unwrap();

        buff
    }

    pub fn unit(&self) -> String {
        match ENDPOINT_KINDS.iter().find(|(_i, k, _s, _u)| k == self) {
            Some(e) => e.3.to_string(),
            None => "unknown".to_string(),
        }
    }
}

impl core::str::FromStr for EpKind {
    type Err = &'static str;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        match ENDPOINT_KINDS
            .iter()
            .find(|(_i, _k, s, _u)| src.to_lowercase() == *s.to_lowercase())
        {
            Some(e) => Ok(e.1),
            None => Err("No matching endpoint name found"),
        }
    }
}

impl From<u16> for EpKind {
    fn from(v: u16) -> Self {
        match ENDPOINT_KINDS.iter().find(|(i, _k, _s, _u)| v == *i) {
            Some(e) => e.1,
            None => EpKind::Unknown(v),
        }
    }
}

impl From<&EpKind> for u16 {
    fn from(kind: &EpKind) -> u16 {
        // Handle unknown endpoints
        if let EpKind::Unknown(v) = kind {
            return *v;
        }

        // Otherwise match against known endpoint knids
        for (i, k, _s, _u) in ENDPOINT_KINDS {
            if k == kind {
                return *i;
            }
        }

        unreachable!()
    }
}
