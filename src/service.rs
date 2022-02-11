use core::marker::PhantomData;
use core::fmt::{Display, Debug};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use alloc::vec;

use dsf_core::base::{Body, Parse, Encode, DataBody, PageBody};
use dsf_core::options::Metadata;
use dsf_core::types::*;
use dsf_core::wire::Container;

use crate::endpoint::{self as ep};
use crate::error::IotError;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;

pub trait EndpointContainer: AsRef<ep::Descriptor> {}

#[derive(Debug, Clone)]
//#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
pub struct IotService<EPS = Vec<ep::Descriptor>, META = Vec<(String, String)>> {
    pub id: Id,

    pub secret_key: Option<SecretKey>,

    /// Service endpoint information
    //#[cfg_attr(feature = "structopt", structopt(long, parse(try_from_str = parse_endpoint_descriptor)))]
    pub endpoints: EPS,

    /// Service metadata
    //#[cfg_attr(feature = "structopt", structopt(long, parse(try_from_str = try_parse_key_value)))]
    pub meta: META,
}

pub struct Service2<PRI=Vec<u8>, OPT=Vec<()>, DAT=Vec<u8>> {
    _body: PRI,
    _public_options: OPT,
    _private_options: (),
    data: PhantomData<DAT>
}

impl <PRI, DAT> Service2<PRI, DAT> 
where
    PRI: Parse + Encode,
    DAT: Parse + Encode,
{

}

pub trait Application {
    const APPLICATION_ID: u16;

    type Info: Debug;
    type Data: Debug;
}

impl Application for dsf_core::service::Service {
    const APPLICATION_ID: u16 = 0x0001;

    type Info = IotInfo;
    type Data = IotData;
}

impl <EPS, META> IotService<EPS, META>
where
    EPS: AsRef<ep::Descriptor>,
    META: AsRef<(String, String)>,
{
    /// Create a new IoT service instance
    pub fn new(id: Id, endpoints: EPS, meta: META) -> Self {
        Self {
            id, secret_key: None, endpoints, meta
        }
    }
}

impl IotService {
    pub fn decode_page(mut p: Container, secret_key: Option<&SecretKey>) -> Result<IotService, IotError> {

        if let Some(sk) = secret_key {
            p.decrypt(sk)?;
        }

        // TODO: refactor this out in a useful way
        let endpoints = match p.encrypted() {
            true => return Err(IotError::NoSecretKey),
            false => IotService::decode_body(&p.body_raw())?,
        };

        // TODO: pass through metadata
        let s = IotService {
            id: p.id(),
            secret_key: secret_key.map(|sk| sk.clone() ),
            endpoints,
            meta: vec![],
        };

        Ok(s)
    }

    pub fn encode_body(
        endpoints: &[ep::Descriptor],
        buff: &mut [u8],
    ) -> Result<usize, IotError> {
        let mut index = 0;

        // Encode each endpoint entry
        for ed in endpoints {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(index)
    }

    pub fn decode_body(buff: &[u8]) -> Result<Vec<ep::Descriptor>, IotError> {
        let mut index = 0;
        let mut endpoints = vec![];

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = ep::Descriptor::parse(&buff[index..])?;

            endpoints.push(ed);
            index += n;
        }

        Ok(endpoints)
    }
}

#[derive(Debug, Clone)]
pub struct IotInfo<C: stor::Stor<Metadata> = stor::Owned, D: AsRef<[ep::Descriptor<C>]> + Debug = Vec<ep::Descriptor<C>>> {
    pub descriptors: D,
    _c: PhantomData<C>,
}

impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Descriptor<C>]> + Debug> IotInfo<C, D> {
    pub fn new(descriptors: D) -> Self {
        Self{ descriptors, _c: PhantomData }
    }
}

impl <'a, C: stor::Stor<Metadata>> IotInfo<C, &'a [ep::Descriptor<C>]> {
    pub fn from_slice(descriptors: &'a [ep::Descriptor<C>]) -> Self {
        Self{ descriptors, _c: PhantomData }
    }
}

impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Descriptor<C>]> + Debug> From<D> for IotInfo<C, D> {
    fn from(descriptors: D) -> Self {
        Self{ descriptors, _c: PhantomData }
    }
}

/// PageBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Descriptor<C>]> + Debug> PageBody for IotInfo<C, D> {}

impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Descriptor<C>]> + Default + Debug> Default for IotInfo<C, D> {
    fn default() -> Self {
        Self { descriptors: Default::default(), _c: Default::default() }
    }
}

impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Descriptor<C>]> + Debug> Encode for IotInfo<C, D> {
    type Error = IotError;

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        let mut index = 0;

        // Encode each endpoint entry
        for ed in self.descriptors.as_ref() {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(index)
    }
}

impl Parse for IotInfo<stor::Owned> {
    type Output = IotInfo<stor::Owned>;

    type Error = IotError;

    fn parse(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;
        let mut descriptors = vec![];

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = ep::Descriptor::parse(&buff[index..])?;

            descriptors.push(ed);
            index += n;
        }

        Ok((IotInfo{descriptors, _c: PhantomData}, index))
    }
}

#[derive(Debug, Clone)]
pub struct IotData<C: stor::Stor<Metadata> = stor::Owned, D: AsRef<[ep::Data<C>]> + Debug = Vec<ep::DataOwned>> {
    /// Measurement values (these must correspond with service endpoints)
    pub data: D,

    _c: PhantomData<C>,
}

impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Data<C>]> + Debug> IotData<C, D> {
    pub fn new(data: D) -> Self {
        Self{ data, _c: PhantomData }
    }
}

impl <'a, C: stor::Stor<Metadata>> IotData<C, &'a [ep::Data<C>]> {
    pub fn from_slice(data: &'a [ep::Data<C>]) -> Self {
        Self{ data, _c: PhantomData }
    }
}

/// DataBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Data<C>]> + Debug> DataBody for IotData<C, D> {}

impl <C: stor::Stor<Metadata>, D: AsRef<[ep::Data<C>]> + Debug> Encode for IotData<C, D> {
    type Error = IotError;

    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        let mut index = 0;

        // Encode each endpoint entry
        for ed in self.data.as_ref() {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(index)
    }
}

impl Parse for IotData<stor::Owned> {
    type Output = IotData<stor::Owned>;

    type Error = IotError;

    fn parse(buff: &[u8]) -> Result<(Self::Output, usize), Self::Error> {
        let mut index = 0;
        let mut data = vec![];

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = ep::Data::parse(&buff[index..])?;

            data.push(ed);
            index += n;
        }

        Ok((IotData{data, _c: PhantomData}, index))
    }
}

#[cfg(test)]
mod test {

}
