use std::time::Duration;

extern crate serde;

extern crate bytes;

extern crate byteorder;

extern crate structopt;

extern crate humantime;

extern crate futures;
use futures::prelude::*;

extern crate dsf_core;
use dsf_core::api::ServiceHandle;
use dsf_core::prelude::*;
use dsf_core::types::DataKind;

extern crate dsf_client;
use dsf_client::prelude::*;
pub use dsf_client::Error;

extern crate dsf_rpc;
use dsf_rpc::{self as rpc, LocateInfo, PublishInfo, ServiceIdentifier, ServiceInfo};

pub mod endpoint;

pub mod options;
pub use options::*;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;

/// IotClient wraps a `dsf_client::Client` and provides interfaces to interact with DSF-IoT services
/// TODO: one day this could be an extension trait?
pub struct IotClient {
    client: Client,
}

impl IotClient {
    /// Create a new DSF-IoT client using the provided path
    pub fn new(path: &str, timeout: Duration) -> Result<Self, ClientError> {
        let client = Client::new(path, timeout)?;

        Ok(Self { client })
    }

    /// Access base client object
    pub fn base(&mut self) -> &mut Client {
        &mut self.client
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

    /// Search for an existing IoT service
    pub async fn search(&mut self, id: &Id) -> Result<(ServiceHandle, LocateInfo), ClientError> {
        self.client
            .locate(LocateOptions {
                id: id.clone(),
                local_only: false,
            })
            .await
    }

    /// List known IoT services
    pub async fn list(&mut self, _options: ListOptions) -> Result<Vec<ServiceInfo>, ClientError> {
        let req = rpc::service::ListOptions {
            application_id: Some(IOT_APP_ID),
        };

        self.client.list(req).await
    }

    /// Register an existing service in the database
    pub async fn register(
        &mut self,
        options: RegisterOptions,
    ) -> Result<dsf_rpc::service::RegisterInfo, ClientError> {
        self.client.register(options).await
    }

    /// Publish an existing IoT service
    pub async fn publish_raw(
        &mut self,
        service: ServiceIdentifier,
        kind: DataKind,
        data: &[u8],
    ) -> Result<PublishInfo, ClientError> {
        let p = dsf_rpc::data::PublishOptions {
            service,
            kind: kind.into(),
            data: Some(data.to_vec()),
        };

        self.client.publish(p).await
    }

    pub async fn publish(&mut self, _options: PublishOptions) -> Result<PublishInfo, ClientError> {
        unimplemented!()
    }

    /// Query for data from an IoT service
    pub async fn query(&mut self, _options: QueryOptions) -> Result<ServiceHandle, ClientError> {
        unimplemented!()
    }

    /// Subscribe to data from an IoT service
    pub async fn subscribe(
        &mut self,
        options: rpc::SubscribeOptions,
    ) -> Result<impl Stream<Item = ()>, ClientError> {
        let resp = self.client.subscribe(options).await?;

        // TODO: decode endpoint info here
        Ok(resp.map(|_d| ()))
    }
}
