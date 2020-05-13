use std::time::Duration;
use std::convert::{TryInto};

extern crate serde;

#[macro_use]
extern crate log;

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
use dsf_core::options::{OptionsError};


extern crate dsf_client;
use dsf_client::prelude::*;
pub use dsf_client::Error;

extern crate dsf_rpc;
use dsf_rpc::{self as rpc, LocateInfo, PublishInfo, ServiceIdentifier, ServiceInfo};

pub mod endpoint;
pub use endpoint::*;

pub mod options;
pub use options::*;

pub mod service;
pub use service::*;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;

/// IotClient wraps a `dsf_client::Client` and provides interfaces to interact with DSF-IoT services
/// TODO: one day this could be an extension trait?
pub struct IotClient {
    client: Client,
}

#[derive(Debug)]
pub enum IotError {
    Client(ClientError),
    Options(OptionsError),
}

impl From<ClientError> for IotError {
    fn from(e: ClientError) -> Self {
        Self::Client(e)
    }
}

impl From<OptionsError> for IotError {
    fn from(o: OptionsError) -> Self {
        Self::Options(o)
    }
}

impl IotClient {
    /// Create a new DSF-IoT client using the provided path
    pub fn new(path: &str, timeout: Duration) -> Result<Self, IotError> {
        let client = Client::new(path, timeout)?;

        Ok(Self { client })
    }

    /// Access base client object
    pub fn base(&mut self) -> &mut Client {
        &mut self.client
    }

    /// Create a new IoT service
    pub async fn create(&mut self, options: CreateOptions) -> Result<ServiceHandle, IotError> {

        info!("Creating service: {:?}", options.service);

        let encoded = options.try_into()?;

        info!("Encoded service info");

        let r = self.client.create(encoded).await?;

        info!("Result: {:?}", r);

        Ok(r)
    }

    /// Search for an existing IoT service
    pub async fn search(&mut self, id: &Id) -> Result<(ServiceHandle, LocateInfo), IotError> {
        let r = self.client
            .locate(LocateOptions {
                id: id.clone(),
                local_only: false,
            })
            .await?;
        Ok(r)
    }

    /// List known IoT services
    pub async fn list(&mut self, _options: ListOptions) -> Result<Vec<ServiceInfo>, IotError> {
        let req = rpc::service::ListOptions {
            application_id: Some(IOT_APP_ID),
        };

        let r = self.client.list(req).await?;
        Ok(r)
    }

    /// Register an existing service in the database
    pub async fn register(
        &mut self,
        options: RegisterOptions,
    ) -> Result<dsf_rpc::service::RegisterInfo, IotError> {
        let r = self.client.register(options).await?;
        Ok(r)
    }

    /// Publish raw data using an existing IoT service
    pub async fn publish_raw(
        &mut self,
        service: ServiceIdentifier,
        kind: DataKind,
        data: &[u8],
    ) -> Result<PublishInfo, IotError> {
        let p = dsf_rpc::data::PublishOptions {
            service,
            kind: kind.into(),
            data: Some(data.to_vec()),
        };

        let r = self.client.publish(p).await?;
        Ok(r)
    }

    pub async fn publish(&mut self, options: PublishOptions) -> Result<PublishInfo, IotError> {
        info!("Publishing data: {:?}", options);

        let encoded = options.try_into()?;

        info!("Encoded service data");

        let r = self.client.publish(encoded).await?;

        info!("Result: {:?}", r);

        Ok(r)
    }

    /// Query for data from an IoT service
    pub async fn query(&mut self, options: QueryOptions) -> Result<Vec<IotData>, IotError> {
        info!("Querying for data: {:?}", options);

        let mut data = self.client.data(options).await?;

        let iot_data = data.drain(..).filter_map(|v| {
            if let Body::Cleartext(b) = v.body {
                let d = IotData::decode_data(&b).unwrap();

                // TODO: pass metadata through here
                return Some(IotData::new(&d, &[]))
            }
            None
        }).collect();

        info!("Result: {:#?}", iot_data);

        Ok(iot_data)
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
