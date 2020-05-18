use std::fmt::Write;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Available endpoint descriptors, their names, units, and IDs
pub const ENDPOINT_KINDS: &[(EndpointKind, &str, &str, u16)] = &[
    (EndpointKind::Temperature, "temperature", "°C", 1),
    (EndpointKind::Humidity, "humidity", "% RH", 2),
    (EndpointKind::Pressure, "pressure", "   kPa", 3),
];

/// Endpoint Kind specifies the type of IoT endpoint. For example,
/// Temperature, Heart-Rate
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum EndpointKind {
    /// Temperature in (degrees Celcius)
    Temperature,
    /// Humidity (in % RH)
    Humidity,
    /// Pressure (in kPa)
    Pressure,
    /// Unknown measurement kind (no units)
    Unknown(u16),
}

/// Parse an endpoint kind from a string
pub fn parse_endpoint_kind(src: &str) -> Result<EndpointKind, String> {
    // Coerce to lower case
    let src = src.to_lowercase();

    // Attempt to find matching endpoint name
    if let Ok(v) = EndpointKind::from_str(&src) {
        return Ok(v);
    }

    // Attempt to parse as an integer
    if let Ok(v) = u16::from_str(&src) {
        return Ok(EndpointKind::Unknown(v));
    }

    Err(format!(
        "Unrecognised endpoint kind '{}' (options: {})",
        src,
        EndpointKind::variants()
    ))
}

impl EndpointKind {
    /// List available endpoint variants
    pub fn variants() -> String {
        let mut buff = String::new();

        for (_k, s, u, i) in ENDPOINT_KINDS {
            write!(&mut buff, "{} (unit: {}, id: {}), ", s, u, i).unwrap();
        }

        write!(&mut buff, "RAW_ID (no unit)").unwrap();

        buff
    }

    pub fn unit(&self) -> String {
        match ENDPOINT_KINDS.iter().find(|(k, _s, _u, _i)| k == self) {
            Some(e) => e.2.to_string(),
            None => "unknown".to_string(),
        }
    }
}

impl core::str::FromStr for EndpointKind {
    type Err = String;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        match ENDPOINT_KINDS.iter().find(|(_k, s, _u, _i)| src == *s) {
            Some(e) => Ok(e.0),
            None => Err(format!("No matching endpoint name found")),
        }
    }
}

impl From<u16> for EndpointKind {
    fn from(v: u16) -> Self {
        match ENDPOINT_KINDS.iter().find(|(_k, _s, _u, i)| v == *i) {
            Some(e) => e.0,
            None => EndpointKind::Unknown(v),
        }
    }
}

impl From<&EndpointKind> for u16 {
    fn from(kind: &EndpointKind) -> u16 {
        // Handle unknown endpoints
        if let EndpointKind::Unknown(v) = kind {
            return *v;
        }

        // Otherwise match against known endpoint knids
        for (k, _s, _u, i) in ENDPOINT_KINDS {
            if k == kind {
                return *i;
            }
        }

        unreachable!()
    }
}
