use core::convert::TryInto;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use bytes::BytesMut;

use clap::{Parser, Subcommand};
use dsf_core::{api::Application, types::Id};

use dsf_core::base::Encode;
use dsf_core::keys::Keys;
use dsf_core::options::Options;
use dsf_core::types::PageKind;

pub use dsf_rpc::service::{try_parse_key_value, LocateOptions, RegisterOptions, SubscribeOptions, ServiceListOptions};
use dsf_rpc::ServiceIdentifier;

use crate::{
    endpoint::{
        parse_endpoint_data, parse_endpoint_descriptor, EpData, EpDescriptor, EpKind, IotData,
    },
    error::IotError,
    IoT,
};

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Create a new IOT service on this device
    Create(CreateOptions),

    /// Register an owned IoT service
    Register(RegisterOptions),

    /// Publish IoT data for an owned service
    Publish(PublishOptions),

    /// Locate an IoT service by SID using the DHT
    Locate(LocateOptions),

    /// Discover IoT services on the local network
    Discover(DiscoverOptions),

    /// Fetch information for a known IoT service
    Info(InfoOptions),

    /// Subscribe to a known IoT service
    Subscribe(SubscribeOptions),

    /// Query for data from a known IoT service
    Data(QueryOptions),

    /// List known IoT services
    List(ListOptions),

    /// Generate a service ID / key for manual loading
    GenKeys,

    /// Helper to encode IoT data objects
    #[clap(hide=true)]
    Encode(EncodeOptions),

    /// Helper to decode IoT data objects
    #[clap(hide=true)]
    Decode(DecodeOptions),

    /// Register an IoT service using a provided Name Service
    NsRegister(NsRegisterOptions),

    /// Search for an IoT service using a Name Service
    NsSearch(NsSearchOptions),
}

#[derive(Debug, Clone, Parser)]
pub struct CreateOptions {
    /// Service endpoint information
    #[clap(long, value_parser=parse_endpoint_descriptor)]
    pub endpoints: Vec<EpDescriptor>,

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
        let n = self.endpoints.encode_len()?;
        let mut body = vec![0u8; n];
        let n = self.endpoints.encode(&mut body[..])?;

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
    pub data: Vec<EpData>,

    /// Measurement metadata
    #[clap(long, value_parser=try_parse_key_value)]
    pub meta: Vec<(String, String)>,
}

impl TryInto<dsf_rpc::PublishOptions> for PublishOptions {
    type Error = IotError;

    // Generate an RPC create message for an IoT service instance
    fn try_into(self) -> Result<dsf_rpc::PublishOptions, Self::Error> {
        let mut body = BytesMut::new();

        let data = IotData::<8>::new(&self.data).map_err(|_| IotError::Overrun)?;

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
pub type QueryOptions = dsf_rpc::data::DataListOptions;

/// ListOptions used to list known iot services
pub type ListOptions = dsf_rpc::service::ServiceListOptions;

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
pub struct MetaOptions {}

impl Into<Vec<Options>> for MetaOptions {
    fn into(self) -> Vec<Options> {
        todo!()
    }
}

#[derive(Debug, Clone, Parser)]
pub struct DiscoverOptions {
    /// Endpoints for filtering
    #[clap(long)]
    pub endpoints: Vec<EpKind>,

    /// Options for filtering
    #[clap(long)]
    pub options: Vec<Options>,
}

#[derive(Debug, Clone, Parser)]
pub struct NsRegisterOptions {
    #[clap(flatten)]
    pub ns: ServiceIdentifier,

    /// IoT service to be registered
    #[clap()]
    pub target: Id,

    /// Name for registration
    #[clap(long)]
    pub name: String,
}

#[derive(Debug, Clone, Parser)]
pub struct NsSearchOptions {
    #[clap(flatten)]
    pub ns: ServiceIdentifier,

    /// Service name filter
    #[clap(long, group = "filters")]
    pub name: Option<String>,

    /// Endpoint filter
    #[clap(long, group = "filters")]
    pub endpoint: Option<EpKind>,

    /// Option filter
    #[clap(long, group = "filters")]
    pub options: Option<Options>,
}
