use core::convert::TryFrom;
use std::marker::PhantomData;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use alloc::vec;

use dsf_core::base::{Body, Parse, Encode};
use dsf_core::page::Page;
use dsf_core::types::*;

#[cfg(feature = "dsf-rpc")]
use dsf_rpc::service::{try_parse_key_value, ServiceInfo};

#[cfg(feature = "dsf-rpc")]
use dsf_rpc::data::DataInfo;

use crate::endpoint::{self as ep, parse_endpoint_descriptor};
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

impl Application<ep::Descriptor, ep::Data> for dsf_core::service::Service {
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
pub struct IotData {
    pub index: u16,
    pub signature: Signature,
    pub previous: Option<Signature>,

    /// Measurement values (these must correspond with service endpoints)
    pub data: Vec<ep::Data>,

    /// Measurement metadata
    pub meta: Vec<(String, String)>,
}

impl IotData {
    // TODO: remove this, duplicates decode_page but worse for RPC
    #[cfg(feature = "dsf-rpc")]
    pub fn decode(mut i: DataInfo, secret_key: Option<&SecretKey>) -> Result<IotData, IotError> {

        if let Some(sk) = &secret_key {

        }

        let data = match &i.body {
            Body::Cleartext(b) => IotData::decode_data(&b)?,
            Body::Encrypted(_e) => return Err(IotError::NoSecretKey),
            Body::None => return Err(IotError::NoBody),
        };

        // TODO: pass through metadata
        let s = IotData {
            index: i.index,
            signature: i.signature,
            previous: i.previous,
            data,
            meta: vec![],
        };

        Ok(s)
    }

    pub fn decode_page(mut p: Page, secret_key: Option<&SecretKey>) -> Result<IotData, IotError> {

        if let Some(sk) = &secret_key {
            p.decrypt(sk)?;
        }

        let data = match &p.body {
            Body::Cleartext(b) => IotData::decode_data(&b)?,
            Body::Encrypted(_e) => return Err(IotError::NoSecretKey),
            Body::None => return Err(IotError::NoBody),
        };

        // TODO: pass through metadata
        let s = IotData {
            index: p.header.index,
            signature: p.signature.unwrap(),
            previous: p.previous_sig,
            data,
            meta: vec![],
        };

        Ok(s)
    }

    pub fn encode_data(data: &[ep::Data], buff: &mut [u8]) -> Result<usize, IotError> {
        let mut index = 0;

        // Encode each endpoint entry
        for ed in data {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(index)
    }

    pub fn decode_data(buff: &[u8]) -> Result<Vec<ep::Data>, IotError> {
        let mut index = 0;
        let mut data = vec![];

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = ep::Data::parse(&buff[index..])?;

            data.push(ed);
            index += n;
        }

        Ok(data)
    }
}
