
use std::time::Duration;

extern crate serde;

extern crate bytes;

extern crate structopt;

extern crate humantime;

extern crate futures;
use futures::prelude::*;

extern crate dsf_core;
use dsf_core::types::DataKind;
use dsf_core::api::{ServiceHandle, Create as _, Locate as _, Subscribe as _, Publish as _, Register as _};

extern crate dsf_client;
use dsf_client::Client;
pub use dsf_client::Error;

extern crate dsf_rpc;
use dsf_rpc::{self as rpc, ServiceInfo, PublishInfo, LocateInfo};

pub mod endpoint;

pub mod commands;
pub use commands::*;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;


/// IotClient wraps a `dsf_client::Client` and provides interfaces to interact with DSF-IoT services
pub struct IotClient {
    client: Client,
}


impl IotClient {
    /// Create a new DSF-IoT client using the provided path
    pub fn new(path: &str, timeout: Duration) -> Result<Self, Error> {
        let client = Client::new(path, timeout)?;

        Ok(Self{client})
    }

    /// Create a new IoT service
    pub async fn create(&mut self, options: CreateOptions) -> Result<ServiceHandle, Error> {

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
    pub async fn register(&mut self, options: RegisterOptions) -> Result<dsf_rpc::service::RegisterInfo, Error> {
        self.client.register(options).await
    }

    /// Publish an existing IoT service
    pub async fn publish(&mut self, options: PublishOptions) -> Result<PublishInfo, Error> {
        
        let req = rpc::data::PublishOptions {
            service: options.service,
            kind: Some(DataKind::Unknown(IOT_DATA_PAGE_KIND)),
            // TODO: encoded body here
            data: None,
        };

        self.client.publish(req).await
    }

    /// Search for an existing IoT service
    pub async fn search(&mut self, options: SearchOptions) -> Result<LocateInfo, Error> {
        let req = rpc::service::LocateOptions{
            id: options.id,
        };

        self.client.locate(req).await
    }

    /// List known IoT services
    pub async fn list(&mut self, _options: ListOptions) -> Result<Vec<ServiceInfo>, Error> {
        let req = rpc::service::ListOptions {
            application_id: Some(IOT_APP_ID),
        };

        self.client.list(req).await
    }

    /// Query for data from an IoT service
    pub async fn query(&mut self, _options: QueryOptions) -> Result<ServiceHandle, Error> {
        unimplemented!()
    }

    /// Subscribe to data from an IoT service
    pub async fn subscribe(&mut self, options: SubscribeOptions) -> Result<impl Stream<Item=()>, Error> {
        
        let req = rpc::service::SubscribeOptions {
            service: options.service,
        };

        let resp = self.client.subscribe(req).await?;

        // TODO: decode endpoint info here
        Ok(resp.map(|_d| () ))
    }

}


