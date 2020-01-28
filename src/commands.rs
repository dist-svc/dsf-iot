

use structopt::StructOpt;

use dsf_core::types::{Id};
use dsf_rpc::ServiceIdentifier;
use dsf_rpc::service::{try_parse_key_value};

pub use dsf_rpc::service::RegisterOptions;

use crate::endpoint::*;

#[derive(Debug, Clone, StructOpt)]
pub enum Command {
    /// Create a new IOT service
    Create(CreateOptions),

    /// Register an IoT service
    Register(RegisterOptions),

    /// Publish IoT data for an owned service
    Publish(PublishOptions),

    /// Search for an IoT service
    Search(SearchOptions),

    /// Subscribe to a known IoT service
    Subscribe(SubscribeOptions),

    /// Query for data from a known IoT service
    Query(QueryOptions),

    /// List known IoT services
    List(ListOptions),
}

#[derive(Debug, Clone, StructOpt)]
pub struct CreateOptions {
    /// Endpoint kinds
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
}

#[derive(Debug, Clone, StructOpt)]
pub struct SearchOptions {
    #[structopt(short = "i", long = "id")]
    /// Service ID
    pub id: Id,
}

#[derive(Debug, Clone, StructOpt)]
pub struct SubscribeOptions {
    #[structopt(flatten)]
    pub service: ServiceIdentifier,
}

#[derive(Debug, Clone, StructOpt)]
pub struct QueryOptions {
    #[structopt(flatten)]
    pub service: ServiceIdentifier,
}

#[derive(Debug, Clone, StructOpt)]
pub struct ListOptions {
    #[structopt(long)]
    /// Offset in service index
    pub offset: Option<usize>,

    #[structopt(long)]
    /// Limit number of returned services
    pub limit: Option<usize>,
}

