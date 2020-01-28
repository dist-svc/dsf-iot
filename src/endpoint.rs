

use std::str::FromStr;
use std::fmt::Write;

use serde::{Serialize, Deserialize};

use structopt::{StructOpt};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointDescriptor {
    /// Index in the IoT service
    pub index: u16,

    /// Endpoint Data Kind
    pub endpoints: Vec<EndpointKind>,
}

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

const ENDPOINT_KINDS: &[(EndpointKind, &str, &str, u16)] = &[
    (EndpointKind::Temperature, "temperature", "C", 1),
    (EndpointKind::Humidity, "humidity", "% RH", 2),
    (EndpointKind::Pressure, "pressure", "kPa", 3),
];


pub fn parse_endpoint_kind(src: &str) -> Result<EndpointKind, String> {
    let src = src.to_lowercase();

    let m = ENDPOINT_KINDS.iter().find(|(_k, s, _u, _i)| src == *s );

    if let Some(e) = m {
        return Ok(e.0);
    }

    if let Ok(v) = u16::from_str(&src) {
        return Ok(EndpointKind::Unknown(v))
    }
    
    Err(format!("Unrecognized endpoint kind '{}' (options: {})", src, EndpointKind::variants()))
}

impl EndpointKind {
    pub fn variants() -> String {
        let mut buff = String::new();

        for (_k, s, u, i) in ENDPOINT_KINDS {
            write!(&mut buff, "{} (unit: {}, id: {}), ", s, u, i).unwrap();
        }

        write!(&mut buff, "RAW_ID (no unit)").unwrap();

        buff
    }
}



/// Data kind identifier
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, StructOpt)]
pub enum DataKind {
    Bool,
    Float32,
    Text,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, StructOpt)]
pub struct EndpointData {
    pub index: u16,
}

