


pub mod kinds;
use dsf_core::{base::{PageBody, DataBody}, prelude::{Encode, Parse}};
pub use kinds::*;

pub mod value;
pub use value::*;

pub mod desc;
pub use desc::*;

use crate::prelude::IotError;

const MAX_EPS: usize = 10;

#[derive(Debug, Clone)]
pub struct IotInfo {
    pub descriptors: heapless::Vec<Descriptor, MAX_EPS>,
}

impl IotInfo {
    pub fn new(descriptors: &[Descriptor]) -> Result<Self, ()> {
        Ok(Self{ descriptors: heapless::Vec::from_slice(descriptors)? })
    }
}

/// PageBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl PageBody for IotInfo {}

impl core::fmt::Display for IotInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.descriptors.len() {
            let e = &self.descriptors[i];
            writeln!(f, "  - {:2}: {:16} in {:4} (metadata: {:?})", i, e.kind, e.kind.unit(), e.meta)?;
        }
        Ok(())
    }
}

impl Default for IotInfo {
    fn default() -> Self {
        Self { descriptors: Default::default() }
    }
}

impl Encode for IotInfo {
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

impl Parse for IotInfo{
    type Output = IotInfo;
    type Error = IotError;

    fn parse(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;
        let mut descriptors = heapless::Vec::new();

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = Descriptor::parse(&buff[index..])?;

            descriptors.push(ed);
            index += n;
        }

        Ok((IotInfo{descriptors, }, index))
    }
}

#[derive(Debug, Clone)]
pub struct IotData {
    /// Measurement values (these must correspond with service endpoints)
    pub data: heapless::Vec<Data, MAX_EPS>,
}

impl IotData {
    pub fn new(data: &[Data]) -> Result<Self, ()> {
        Ok(Self{ data: heapless::Vec::from_slice(data)? })
    }
}


/// DataBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl DataBody for IotData {}

impl core::fmt::Display for IotData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.data.len() {
            let e = &self.data[i];
            writeln!(f, "  - {:2}: {:4} (metadata: {:?})", i, e.value, e.meta)?;
        }
        Ok(())
    }
}

impl Encode for IotData {
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

impl Parse for IotData {
    type Output = IotData;

    type Error = IotError;

    fn parse(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;
        let mut data = heapless::Vec::new();

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = Data::parse(&buff[index..])?;

            data.push(ed);
            index += n;
        }

        Ok((IotData{data}, index))
    }
}
