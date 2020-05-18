use std::convert::TryFrom;

use structopt::StructOpt;

use dsf_rpc::service::{ServiceInfo, try_parse_key_value};
use dsf_rpc::data::{DataInfo};

use dsf_core::types::*;
use dsf_core::base::{Body};

use crate::IotError;
use crate::endpoint::*;


pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;


#[derive(Debug, Clone, StructOpt)]
pub struct IotService {
    pub id: Id,

    pub secret_key: Option<SecretKey>,

    /// Service endpoint information
    #[structopt(long, parse(try_from_str=parse_endpoint_descriptor))]
    pub endpoints: Vec<EndpointDescriptor>,

    /// Service metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub meta: Vec<(String, String)>,
}

impl TryFrom<ServiceInfo> for IotService {
    type Error = IotError;

    fn try_from(mut i: ServiceInfo) -> Result<IotService, IotError> {
        
        i.body.decrypt(i.secret_key.as_ref()).unwrap();

        let endpoints = match &i.body {
            Body::Cleartext(b) => IotService::decode_body(b)?,
            Body::Encrypted(_e) => return Err(IotError::NoSecretKey),
            Body::None => return Err(IotError::NoBody),
        };

        // TODO: pass through metadata
        let s = IotService {
            id: i.id,
            secret_key: i.secret_key.clone(),
            endpoints,
            meta: vec![],
        };

        Ok(s)
    }
}


impl IotService {

    pub fn encode_body(endpoints: &[EndpointDescriptor]) -> Result<Vec<u8>, IotError> {
        let mut buff = vec![0u8; 1024];
        let mut index = 0;

        // Encode each endpoint entry
        for ed in endpoints {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(buff[0..index].to_vec())
    }

    pub fn decode_body(buff: &[u8]) -> Result<Vec<EndpointDescriptor>, IotError> {
        let mut index = 0;
        let mut endpoints = vec![];

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = EndpointDescriptor::parse(&buff[index..])?;

            endpoints.push(ed);
            index += n;
        }

        Ok(endpoints)
    }
}


#[derive(Debug, Clone)]
pub struct IotData {
    pub signature: Signature,
    pub previous: Option<Signature>,

    /// Measurement values (these must correspond with service endpoints)
    pub data: Vec<EndpointData>,

    /// Measurement metadata
    pub meta: Vec<(String, String)>,
}

impl IotData {
    pub fn decode(mut i: DataInfo, secret_key: Option<&SecretKey>) -> Result<IotData, IotError> {
        
        i.body.decrypt(secret_key).unwrap();

        let data = match &i.body {
            Body::Cleartext(b) => IotData::decode_data(b)?,
            Body::Encrypted(_e) => return Err(IotError::NoSecretKey),
            Body::None => return Err(IotError::NoBody),
        };

        // TODO: pass through metadata
        let s = IotData {
            signature: i.signature,
            previous: i.previous,
            data,
            meta: vec![],
        };

        Ok(s)
    }

    pub fn encode_data(data: &[EndpointData]) -> Result<Vec<u8>, IotError> {
        let mut buff = vec![0u8; 1024];
        let mut index = 0;

        // Encode each endpoint entry
        for ed in data {
            index += ed.encode(&mut buff[index..])?;
        }

        Ok(buff[0..index].to_vec())
    }

    pub fn decode_data(buff: &[u8]) -> Result<Vec<EndpointData>, IotError> {
        let mut index = 0;
        let mut data = vec![];

        // Decode each endpoint entry
        while index < buff.len() {
            let (ed, n) = EndpointData::parse(&buff[index..])?;

            data.push(ed);
            index += n;
        }

        Ok(data)
    }
}
