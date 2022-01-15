use core::convert::TryFrom;
use core::marker::PhantomData;
use core::fmt::Debug;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use alloc::vec;

use dsf_core::base::{Body, Parse, Encode, DataBody};
use dsf_core::options::Metadata;
use dsf_core::page::Page;
use dsf_core::types::*;

#[cfg(feature = "dsf-rpc")]
use dsf_rpc::service::{try_parse_key_value, ServiceInfo};

#[cfg(feature = "dsf-rpc")]
use dsf_rpc::data::DataInfo;

use crate::endpoint::{self as ep, parse_endpoint_descriptor, StringIsh, MetaIsh, BytesIsh};
use crate::error::IotError;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;

pub trait EndpointContainer: AsRef<ep::Descriptor> {}

pub trait Idk<Inner>: Debug {
    type Container: AsRef<[Inner]> + Debug;
    type String: AsRef<str> + Debug;
    type Bytes: AsRef<[u8]> + Debug;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct IdkOwned;

impl <T: Debug> Idk<T> for IdkOwned {
    type Container = Vec<T>;
    type String = String;
    type Bytes = Vec<u8>;
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct IdkRef<'a> (PhantomData<&'a ()>);

impl <'a, T: Debug + 'a> Idk<T> for IdkRef<'a> {
    type Container = &'a [T];
    type String = &'a str;
    type Bytes = &'a [u8];
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct IdkConst<const N: usize>;

impl <T: Debug, const N: usize> Idk<T> for IdkConst<N> {
    type Container = [T; N];
    type String = &'static str;
    type Bytes = [u8; N];
}

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
    body: PRI,
    public_options: OPT,
    private_options: (),
    data: PhantomData<DAT>
}

impl <PRI, DAT> Service2<PRI, DAT> 
where
    PRI: Parse + Encode,
    DAT: Parse + Encode,
{

}

pub trait Application<PRI, DAT> {
    const APPLICATION_ID: u16;

}

impl Application<ep::Descriptor, ep::DataOwned> for dsf_core::service::Service {
    const APPLICATION_ID: u16 = 0x0001;

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
    pub fn decode_page(mut p: Page, secret_key: Option<&SecretKey>) -> Result<IotService, IotError> {

        if let Some(sk) = secret_key {
            p.decrypt(sk)?;
        }

        let endpoints = match &p.body {
            Body::Cleartext(b) => IotService::decode_body(&b)?,
            Body::Encrypted(_e) => return Err(IotError::NoSecretKey),
            Body::None => return Err(IotError::NoBody),
        };

        // TODO: pass through metadata
        let s = IotService {
            id: p.id,
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
pub struct IotData<C: Idk<Metadata>, D: AsRef<[ep::Data<C>]> + Debug = Vec<ep::DataOwned>> {
    /// Measurement values (these must correspond with service endpoints)
    pub data: D,

    _c: PhantomData<C>,
}

impl <C: Idk<Metadata>, D: AsRef<[ep::Data<C>]> + Debug> IotData<C, D> {
    pub fn new(data: D) -> Self {
        Self{ data, _c: PhantomData }
    }
}

/// DataBody marker allows this to be used with [`dsf_core::Service::publish_data`]
impl <C: Idk<Metadata>, D: AsRef<[ep::Data<C>]> + Debug> DataBody for IotData<C, D> {}

/// All storage types are 
impl <C: Idk<Metadata>, D: AsRef<[ep::Data<C>]> + Debug> Encode for IotData<C, D> {
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

impl Parse for IotData<IdkOwned> {
    type Output = IotData<IdkOwned>;

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
