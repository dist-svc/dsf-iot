use std::convert::TryInto;
use structopt::StructOpt;

use dsf_core::base::NewBody;

pub use dsf_rpc::service::{try_parse_key_value, LocateOptions, RegisterOptions, SubscribeOptions};
use dsf_rpc::ServiceIdentifier;

use crate::endpoint::*;
use crate::service::*;
use crate::{IotError, IotService};

#[derive(Debug, Clone, StructOpt)]
pub enum Command {
    /// Create a new IOT service
    Create(CreateOptions),

    /// Register an IoT service
    Register(RegisterOptions),

    /// Publish IoT data for an owned service
    Publish(PublishOptions),

    /// Locate an IoT service
    Locate(LocateOptions),

    /// Subscribe to a known IoT service
    Subscribe(SubscribeOptions),

    /// Query for data from a known IoT service
    Query(QueryOptions),

    /// List known IoT services
    List(ListOptions),
}

#[derive(Debug, Clone, StructOpt)]
pub struct CreateOptions {
    /// Service endpoint information
    #[structopt(long, parse(try_from_str=parse_endpoint_descriptor))]
    pub endpoints: Vec<EndpointDescriptor>,

    /// Service metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub meta: Vec<(String, String)>,

    #[structopt(short = "p", long = "public")]
    /// Indicate the service should be public (unencrypted)
    pub public: bool,

    #[structopt(long = "register")]
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
        let body = IotService::encode_body(&self.endpoints)?;

        let co = dsf_rpc::CreateOptions {
            application_id: IOT_APP_ID,
            page_kind: Some(IOT_SERVICE_PAGE_KIND),
            body: Some(NewBody::Cleartext(body)),
            metadata: self.meta.clone(),
            public: self.public,
            register: self.register,
            ..Default::default()
        };

        Ok(co)
    }
}

#[derive(Debug, Clone, StructOpt)]
pub struct PublishOptions {
    #[structopt(flatten)]
    pub service: ServiceIdentifier,

    /// Measurement values (these must correspond with service endpoints)
    #[structopt(short, long, parse(try_from_str = parse_endpoint_data))]
    pub data: Vec<EndpointData>,

    /// Measurement metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub meta: Vec<(String, String)>,
}

impl TryInto<dsf_rpc::PublishOptions> for PublishOptions {
    type Error = IotError;

    // Generate an RPC create message for an IoT service instance
    fn try_into(self) -> Result<dsf_rpc::PublishOptions, Self::Error> {
        let data = IotData::encode_data(&self.data)?;

        let po = dsf_rpc::PublishOptions {
            service: self.service,
            kind: None,
            data: Some(data),
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
