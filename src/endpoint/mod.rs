use encdec::{Decode, DecodeOwned, Encode};
use heapless::Vec;

use dsf_core::base::{DataBody, PageBody};

pub mod kinds;
pub use kinds::*;

pub mod value;
pub use value::*;

pub mod desc;
pub use desc::*;

use crate::prelude::IotError;

/// IoT information object containing endpoint descriptors and service metadata
#[derive(Debug, Encode, DecodeOwned)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[encdec(error = "IotError")]
pub struct IotInfo<const N: usize = 8> {
    pub descriptors: Vec<EpDescriptor, N>,
}

impl<const N: usize> IotInfo<N> {
    /// Create a new [`IotInfo`] object with the provided descriptors
    pub fn new(descriptors: &[EpDescriptor]) -> Result<Self, ()> {
        Ok(Self {
            descriptors: Vec::from_slice(descriptors)?,
        })
    }
}

/// PageBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl<const N: usize> PageBody for IotInfo<N> {}

impl<const N: usize> core::fmt::Display for IotInfo<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.descriptors.len() {
            let e = &self.descriptors[i];
            writeln!(f, "  - {:2}: {:16} in {:4}", i, e.kind, e.kind.unit())?;
        }
        Ok(())
    }
}

impl<const N: usize> Default for IotInfo<N> {
    fn default() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }
}
#[derive(Debug, Encode, DecodeOwned)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[encdec(error = "IotError")]
pub struct IotData<const N: usize = 8> {
    /// Measurement values (these must correspond with service endpoints)
    pub data: Vec<EpData, N>,
}

impl<const N: usize> IotData<N> {
    pub fn new(data: &[EpData]) -> Result<Self, ()> {
        Ok(Self {
            data: Vec::from_slice(data)?,
        })
    }
}

/// DataBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl<const N: usize> DataBody for IotData<N> {}

impl<const N: usize> core::fmt::Display for IotData<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.data.len() {
            let e = &self.data[i];
            writeln!(f, "  - {:2}: {:4}", i, e.value)?;
        }
        Ok(())
    }
}
