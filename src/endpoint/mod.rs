
use heapless::Vec;

pub mod kinds;
use byteorder::{LittleEndian, ByteOrder};
use dsf_core::{base::{PageBody, DataBody}, prelude::{Encode, Parse}};
pub use kinds::*;

pub mod value;
pub use value::*;

pub mod desc;
pub use desc::*;

use crate::prelude::IotError;

/// IoT information object containing endpoint descriptors and service metadata
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IotInfo<const N: usize = 8> {
    pub descriptors: Vec<Descriptor, N>,
}

impl <const N: usize> IotInfo<N> {
    /// Create a new [`IotInfo`] object with the provided descriptors
    pub fn new(descriptors: &[Descriptor]) -> Result<Self, ()> {
        Ok(Self{ 
            descriptors: Vec::from_slice(descriptors)?,
        })
    }
}

/// PageBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl <const N: usize> PageBody for IotInfo<N> {}

impl <const N: usize> core::fmt::Display for IotInfo<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.descriptors.len() {
            let e = &self.descriptors[i];
            writeln!(f, "  - {:2}: {:16} in {:4}", i, e.kind, e.kind.unit())?;
        }
        Ok(())
    }
}

impl <const N: usize> Default for IotInfo<N> {
    fn default() -> Self {
        Self { descriptors: Vec::new() }
    }
}

impl <const N: usize> Encode for IotInfo<N> {
    type Error = IotError;

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        let mut index = 0;

        // Encode each endpoint entry
        for ed in &self.descriptors {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(index)
    }
}

impl <const N: usize> Parse for IotInfo<N> {
    type Output = IotInfo;
    type Error = IotError;

    fn parse(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;
        let mut descriptors = Vec::new();

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = Descriptor::parse(&buff[index..])?;

            descriptors.push(ed);
            index += n;
        }

        Ok((IotInfo{descriptors, }, index))
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IotData<const N: usize = 8> {
    /// Measurement values (these must correspond with service endpoints)
    pub data: Vec<Data, N>,
}

impl <const N: usize> IotData<N> {
    pub fn new(data: &[Data]) -> Result<Self, ()> {
        Ok(Self{
            data: Vec::from_slice(data)?,
        })
    }
}


/// DataBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl <const N: usize> DataBody for IotData<N> {}

impl <const N: usize> core::fmt::Display for IotData<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.data.len() {
            let e = &self.data[i];
            writeln!(f, "  - {:2}: {:4}", i, e.value)?;
        }
        Ok(())
    }
}

impl <const N: usize> Encode for IotData<N> {
    type Error = IotError;

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        let mut index = 0;

        // Encode each endpoint entry
        for ed in &self.data {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(index)
    }
}

impl <const N: usize> Parse for IotData<N> {
    type Output = IotData;

    type Error = IotError;

    fn parse(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;
        let mut data = Vec::new();

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = Data::parse(&buff[index..])?;

            data.push(ed);
            index += n;
        }

        Ok((IotData{data}, index))
    }
}
