#![cfg_attr(not(feature = "std"), no_std)]

#![feature(generic_associated_types)]
#![feature(trait_alias)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

use encdec::DecodeExt;

use dsf_core::api::Application;

pub mod endpoint;
pub mod engine;
use engine::{Engine};
pub mod error;
pub mod service;
pub mod prelude;
#[cfg(feature = "client")]
pub mod client;


/// IoT application marker object
pub struct IoT;

/// IoT application specification
impl Application for IoT {
    /// IoT is the first DSF application
    const APPLICATION_ID: u16 = 1;

    /// IotInfo object contains endpoint descriptors
    type Info = endpoint::IotInfo;

    /// IotData object contains endpoint data
    type Data = endpoint::IotData;

    /// Helper to match our service against a discovery request
    fn matches(body: &Self::Info, req: &[u8]) -> bool {

        // Always match empty requests
        if req.len() == 0 {
            return true;
        }

        // Otherwise check for matching endpoint types
        for e in crate::endpoint::Descriptor::decode_iter(req).filter_map(|d| d.ok() ) {
            if body.descriptors.contains(&e) {
                log::debug!("Filter match on endpoint: {:?}", e);
                return true;
            }
        }

        // Fall through for no matches
        return false;
    }
}

/// IoT type for engine instances
pub type IotEngine<Comms, Stor, const N: usize = 512> = Engine<IoT, Comms, Stor, N>;

#[cfg(feature = "defmt")]
mod log {
    pub use defmt::{trace, debug, info, warn, error};

    pub trait Debug = core::fmt::Debug + defmt::Format;
}

#[cfg(not(feature = "defmt"))]
mod log {
    pub use log::{trace, debug, info, warn, error};

    pub trait Debug = core::fmt::Debug;
}
