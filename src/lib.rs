use std::time::Duration;
use std::convert::{TryInto, TryFrom};

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
use dsf_rpc::{self as rpc, LocateInfo, PublishInfo};

pub use dsf_rpc::ServiceIdentifier;

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
    NoSecretKey,
    NoBody,
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

        debug!("Creating service: {:?}", options.endpoints);

        let encoded = options.try_into()?;

        debug!("Encoded service info");

        let r = self.client.create(encoded).await?;

        debug!("Result: {:?}", r);

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
    pub async fn list(&mut self, _options: ListOptions) -> Result<Vec<IotService>, IotError> {
        let req = rpc::service::ListOptions {
            application_id: Some(IOT_APP_ID),
        };

        let mut r = self.client.list(req).await?;

        let s = r.drain(..).map(|v| IotService::try_from(v).unwrap() ).collect();

        Ok(s)
    }

    /// Register an existing service in the database
    pub async fn register(
        &mut self,
        options: RegisterOptions,
    ) -> Result<dsf_rpc::service::RegisterInfo, IotError> {
        let r = self.client.register(options).await?;
        Ok(r)
    }

    /// Fetch service information
    pub async fn info(
        &mut self,
        options: InfoOptions,
    ) -> Result<IotService, IotError> {
        let (_h, mut i) = self.client.info(options).await?;

        i.body.decrypt(i.secret_key.as_ref()).unwrap();

        IotService::try_from(i)
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
        debug!("Publishing data: {:?}", options);

        let encoded = options.try_into()?;

        debug!("Encoded service data");

        let r = self.client.publish(encoded).await?;

        debug!("Result: {:?}", r);

        Ok(r)
    }

    /// Query for data from an IoT service
    pub async fn query(&mut self, options: QueryOptions) -> Result<(IotService, Vec<IotData>), IotError> {
        debug!("Querying for data: {:?}", options);

        let iot_info = self.info(InfoOptions{service: options.service.clone()}).await?;

        let mut data_info = self.client.data(options).await?;

        let iot_data = data_info.drain(..).map(|v| {
            IotData::decode(v, iot_info.secret_key.as_ref()).unwrap()
        }).collect();

        debug!("Result: {:#?}", iot_data);

        Ok((iot_info, iot_data))
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
