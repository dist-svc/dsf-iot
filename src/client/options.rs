use core::convert::TryInto;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use bytes::BytesMut;

use dsf_core::api::Application;
use clap::{Parser, Subcommand};

use dsf_core::base::{Encode};
use dsf_core::types::{PageKind};
use dsf_core::options::Options;
use dsf_core::keys::Keys;

pub use dsf_rpc::service::{try_parse_key_value, LocateOptions, RegisterOptions, SubscribeOptions};
use dsf_rpc::ServiceIdentifier;

use crate::endpoint::{self as ep, parse_endpoint_descriptor, parse_endpoint_data, IotData};
use crate::error::IotError;
use crate::{service::*, IoT};

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Create a new IOT service
    Create(CreateOptions),

    /// Register an IoT service
    Register(RegisterOptions),

    /// Publish IoT data for an owned service
    Publish(PublishOptions),

    /// Locate an IoT service
    Locate(LocateOptions),

    /// Discover local IoT services
    Discover(DiscoverOptions),

    /// Fetch IoT service information
    Info(InfoOptions),

    /// Subscribe to a known IoT service
    Subscribe(SubscribeOptions),

    /// Query for data from a known IoT service
    Query(QueryOptions),

    /// List known IoT services
    List(ListOptions),

    /// Generate a service ID / key for manual loading
    GenKeys,

    /// Encode iot data objects
    Encode(EncodeOptions),

    /// Decode iot data objects
    Decode(DecodeOptions),
}

#[derive(Debug, Clone, Parser)]
pub struct CreateOptions {
    /// Service endpoint information
    #[clap(long, value_parser=parse_endpoint_descriptor)]
    pub endpoints: Vec<ep::Descriptor>,

    /// Service metadata
    #[clap(long, value_parser=try_parse_key_value)]
    pub meta: Vec<(String, String)>,

    #[clap(short)]
    /// Indicate the service should be public (unencrypted)
    pub public: bool,

    #[clap(long)]
    /// Indicate the service should be registered and replicated following creation
    pub register: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            endpoints: vec![],
            meta: vec![],
            public: false,
            register: true,
        }
    }
}

impl TryInto<dsf_rpc::CreateOptions> for CreateOptions {
    type Error = IotError;

    // Generate an RPC create message for an IoT service instance
    fn try_into(self) -> Result<dsf_rpc::CreateOptions, Self::Error> {
        let mut body = BytesMut::new();

        let n = IotService::encode_body(&self.endpoints, &mut body)?;

        let co = dsf_rpc::CreateOptions {
            application_id: IoT::APPLICATION_ID,
            page_kind: Some(PageKind::Generic),
            body: Some(body[..n].to_vec()),
            metadata: self.meta.clone(),
            public: self.public,
            register: self.register,
            ..Default::default()
        };

        Ok(co)
    }
}

#[derive(Debug, Clone, Parser)]
pub struct PublishOptions {
    #[clap(flatten)]
    pub service: ServiceIdentifier,

    /// Measurement values (these must correspond with service endpoints)
    #[clap(short, long, value_parser=parse_endpoint_data)]
    pub data: Vec<ep::Data>,

    /// Measurement metadata
    #[clap(long, value_parser=try_parse_key_value)]
    pub meta: Vec<(String, String)>,
}

impl TryInto<dsf_rpc::PublishOptions> for PublishOptions {
    type Error = IotError;

    // Generate an RPC create message for an IoT service instance
    fn try_into(self) -> Result<dsf_rpc::PublishOptions, Self::Error> {
        let mut body = BytesMut::new();

        let data = IotData::<8>::new(&self.data)
            .map_err(|_| IotError::Overrun )?;

        let n = data.encode(&mut body)?;

        let po = dsf_rpc::PublishOptions {
            service: self.service,
            kind: 0,
            data: Some((&body[..n]).to_vec()),
        };

        Ok(po)
    }
}

/// QueryOptions used to fetch data for an IoT service
pub type QueryOptions = dsf_rpc::data::ListOptions;

/// ListOptions used to list known iot services
pub type ListOptions = dsf_rpc::service::ListOptions;

/// InfoOptions used to fetch info for services
pub type InfoOptions = dsf_rpc::service::InfoOptions;

#[derive(Debug, Clone, Parser)]
pub struct EncodeOptions {
    #[clap(flatten)]
    pub create: CreateOptions,

    /// Keys for decoding
    #[clap(flatten)]
    pub keys: Keys,

    /// File name to write encoded service
    #[clap(long)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Parser)]
pub struct DecodeOptions {
    /// File name to parse encoded iot data
    #[clap(long)]
    pub file: String,

    /// Keys for decoding
    #[clap(flatten)]
    pub keys: Keys,
}

/// IoT Metadata mapping from clap to an options list
#[derive(Debug, Clone, Parser)]
pub struct MetaOptions {

}

impl Into<Vec<Options>> for MetaOptions {
    fn into(self) -> Vec<Options> {
        todo!()
    }
}

#[derive(Debug, Clone, Parser)]
pub struct DiscoverOptions {

}