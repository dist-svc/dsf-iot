
use std::time::Duration;

extern crate serde;

extern crate bytes;

extern crate structopt;

extern crate humantime;

extern crate futures;
use futures::prelude::*;

extern crate dsf_core;
use dsf_core::prelude::*;
use dsf_core::types::DataKind;
use dsf_core::api::ServiceHandle;

extern crate dsf_client;
use dsf_client::prelude::*;

extern crate dsf_rpc;
use dsf_rpc::{self as rpc, ServiceIdentifier, ServiceInfo, PublishInfo, LocateInfo};

pub mod endpoint;

pub mod options;
pub use options::*;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;


/// IotClient wraps a `dsf_client::Client` and provides interfaces to interact with DSF-IoT services
/// TODO: one day this should be an extension trait
pub struct IotClient {
    client: Client,
}


impl IotClient {
    /// Create a new DSF-IoT client using the provided path
    pub fn new(path: &str, timeout: Duration) -> Result<Self, ClientError> {
        let client = Client::new(path, timeout)?;

        Ok(Self{client})
    }

    /// Create a new IoT service
    pub async fn create(&mut self, options: CreateOptions) -> Result<ServiceHandle, ClientError> {

        let req = rpc::service::CreateOptions {
            application_id: IOT_APP_ID,
            page_kind: Some(IOT_SERVICE_PAGE_KIND),
            // TODO: encoded body here
            body: None,
            metadata: options.metadata,
            public: options.public,
            register: options.register,
            ..Default::default()
        };

        self.client.create(req).await
    }

    /// Register an existing service in the database
    pub async fn register(&mut self, handle: &ServiceHandle) -> Result<dsf_rpc::service::RegisterInfo, ClientError> {
        self.client.register(handle).await
    }

    /// Publish an existing IoT service
    pub async fn publish(&mut self, handle: &ServiceHandle, kind: DataKind, data: &[u8]) -> Result<PublishInfo, ClientError> {
        self.client.publish(handle, Some(kind), Some(data)).await
    }

    /// Search for an existing IoT service
    pub async fn search(&mut self, id: &Id) -> Result<(ServiceHandle, LocateInfo), ClientError> {
        self.client.locate(id).await
    }

    /// List known IoT services
    pub async fn list(&mut self, _options: ListOptions) -> Result<Vec<ServiceInfo>, ClientError> {
        let req = rpc::service::ListOptions {
            application_id: Some(IOT_APP_ID),
        };

        self.client.list(req).await
    }

    /// Query for data from an IoT service
    pub async fn query(&mut self, _options: QueryOptions) -> Result<ServiceHandle, ClientError> {
        unimplemented!()
    }

    /// Subscribe to data from an IoT service
    pub async fn subscribe(&mut self, handle: &ServiceHandle, options: SubscribeOptions) -> Result<impl Stream<Item=()>, ClientError> {
        
        let req = rpc::service::SubscribeOptions {
            service: options.service,
        };

        let resp = self.client.subscribe(handle, req).await?;

        // TODO: decode endpoint info here
        Ok(resp.map(|_d| () ))
    }

}


