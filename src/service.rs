
use structopt::StructOpt;
use bytes::BytesMut;

use dsf_rpc::service::{CreateOptions, try_parse_key_value};
use dsf_rpc::data::{PublishOptions};

use dsf_core::base::NewBody;

use crate::IotError;
use crate::endpoint::*;


pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;


#[derive(Debug, Clone, StructOpt)]
pub struct IotService {
    /// Service endpoint information
    #[structopt(long, parse(try_from_str=parse_endpoint_descriptor))]
    pub endpoints: Vec<EndpointDescriptor>,

    /// Service metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub metadata: Vec<(String, String)>,
}


#[derive(Debug, Clone, StructOpt)]
pub struct IotData {
    /// Measurement values (these must correspond with service endpoints)
    #[structopt(short, long, parse(try_from_str = parse_endpoint_data))]
    pub data: Vec<EndpointData>,

    /// Measurement metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub metadata: Vec<(String, String)>,
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


impl IotData {
    pub fn new(data: &[EndpointData], meta: &[(String, String)]) -> Self {
        Self {
            data: data.to_vec(),
            metadata: meta.to_vec(),
        }
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
