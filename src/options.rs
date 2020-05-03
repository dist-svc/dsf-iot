use structopt::StructOpt;

use dsf_rpc::service::try_parse_key_value;
use dsf_rpc::ServiceIdentifier;

pub use dsf_rpc::service::{LocateOptions, RegisterOptions, SubscribeOptions};
use dsf_rpc::{PageBounds, TimeBounds};

use crate::endpoint::*;

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
    #[structopt(long, parse(try_from_str=parse_endpoint_kind))]
    pub endpoints: Vec<EndpointKind>,

    /// Service metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub metadata: Vec<(String, String)>,

    #[structopt(short = "p", long = "public")]
    /// Indicate the service should be public (unencrypted)
    pub public: bool,

    #[structopt(long = "register")]
    /// Indicate the service should be registered and replicated following creation
    pub register: bool,
}

#[derive(Debug, Clone, StructOpt)]
pub struct PublishOptions {
    #[structopt(flatten)]
    pub service: ServiceIdentifier,

    /// Measurement values (these must correspond with service endpoints)
    #[structopt(short, long, parse(try_from_str = parse_endpoint_value))]
    pub data: Vec<EndpointValue>,

    /// Measurement metadata
    #[structopt(long = "meta", parse(try_from_str = try_parse_key_value))]
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, StructOpt)]
pub struct QueryOptions {
    #[structopt(flatten)]
    pub service: ServiceIdentifier,

    #[structopt(flatten)]
    pub bounds: TimeBounds,
}

#[derive(Debug, Clone, StructOpt)]
pub struct ListOptions {
    #[structopt(flatten)]
    pub bounds: PageBounds,
}
