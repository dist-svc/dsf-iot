use structopt::StructOpt;
use std::convert::TryInto;

use dsf_core::base::NewBody;

use dsf_rpc::{ServiceIdentifier, PageBounds};
pub use dsf_rpc::service::{LocateOptions, RegisterOptions, SubscribeOptions, try_parse_key_value};

use crate::{IotError, IotService};
use crate::endpoint::*;
use crate::service::*;

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
    #[structopt(flatten)]
    pub service: IotService,

    #[structopt(short = "p", long = "public")]
    /// Indicate the service should be public (unencrypted)
    pub public: bool,

    #[structopt(long = "register")]
    /// Indicate the service should be registered and replicated following creation
    pub register: bool,
}

impl TryInto<dsf_rpc::CreateOptions> for CreateOptions {
    type Error = IotError;

    // Generate an RPC create message for an IoT service instance
    fn try_into(self) -> Result<dsf_rpc::CreateOptions, Self::Error> {

        let body = IotService::encode_body(&self.service.endpoints)?;

        let co = dsf_rpc::CreateOptions {
            application_id: IOT_APP_ID,
            page_kind: Some(IOT_SERVICE_PAGE_KIND),
            body: Some(NewBody::Cleartext(body)),
            metadata: self.service.metadata.clone(),
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
    pub metadata: Vec<(String, String)>,
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
