use core::marker::PhantomData;
use core::fmt::{Display, Debug};
use core::convert::TryFrom;

#[cfg(feature = "alloc")]
use alloc::{vec::Vec, string::String};

use encdec::{Encode, Decode, DecodeOwned};

use dsf_core::base::{Body, DataBody, PageBody};
use dsf_core::types::*;
use dsf_core::wire::Container;

use crate::endpoint::{self as ep};
use crate::error::IotError;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;

pub trait EndpointContainer: AsRef<ep::Descriptor> {}

#[derive(Debug, Clone)]
//#[cfg_attr(feature = "clap", derive(clap::StructOpt))]
pub struct IotService<EPS = Vec<ep::Descriptor>, META = Vec<(String, String)>> {
    pub id: Id,

    pub secret_key: Option<SecretKey>,

    /// Service endpoint information
    //#[cfg_attr(feature = "clap", clap(long, parse(try_from_str = parse_endpoint_descriptor)))]
    pub endpoints: EPS,

    /// Service metadata
    //#[cfg_attr(feature = "clap", clap(long, parse(try_from_str = try_parse_key_value)))]
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
    PRI: Encode + DecodeOwned,
    DAT: Encode + DecodeOwned,
{

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
            let (ed, n) = ep::Descriptor::decode(&buff[index..])?;

            endpoints.push(ed);
            index += n;
        }

        Ok(endpoints)
    }
}


#[cfg(test)]
mod test {

}
