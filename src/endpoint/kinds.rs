use std::str::FromStr;
use std::fmt::Write;

use serde::{Serialize, Deserialize};

/// Available endpoint descriptors, their names, units, and IDs
const ENDPOINT_KINDS: &[(EndpointKind, &str, &str, u16)] = &[
    (EndpointKind::Temperature,     "temperature",  "C",    1),
    (EndpointKind::Humidity,        "humidity",     "% RH", 2),
    (EndpointKind::Pressure,        "pressure", "   kPa",   3),
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
    let src = src.to_lowercase();

    // Attempt to find matching endpoint name
    let m = ENDPOINT_KINDS.iter().find(|(_k, s, _u, _i)| src == *s );
    if let Some(e) = m {
        return Ok(e.0);
    }

    // Attempt to parse as an integer
    if let Ok(v) = u16::from_str(&src) {
        return Ok(EndpointKind::Unknown(v))
    }
    
    Err(format!("Unrecognised endpoint kind '{}' (options: {})", src, EndpointKind::variants()))
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
}
