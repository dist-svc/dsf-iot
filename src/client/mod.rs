use core::convert::{TryFrom, TryInto};

use futures::prelude::*;
use log::{debug, info, warn};

use bytes::BytesMut;

#[cfg(feature="alloc")]
use pretty_hex::*;

use dsf_client::prelude::*;
use dsf_rpc::{self as rpc, PublishInfo};

use dsf_core::api::ServiceHandle;
use dsf_core::prelude::*;
use dsf_core::types::DataKind;

pub use dsf_client::{Error, Options};
pub use dsf_rpc::ServiceIdentifier;
use rpc::FetchOptions;

use crate::error::IotError;
use crate::service::*;

pub mod options;
pub use options::*;

/// IotClient wraps a `dsf_client::Client` and provides interfaces to interact with DSF-IoT services
/// TODO: one day this could be an extension trait?
pub struct IotClient {
    client: Client,
}

impl IotClient {
    /// Create a new DSF-IoT client using the provided path
    pub fn new(options: &Options) -> Result<Self, IotError> {
        let client = Client::new(options)?;

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
    pub async fn search(&mut self, id: &Id) -> Result<(ServiceHandle, IotService), IotError> {
        let (h, _i) = self
            .client
            .locate(LocateOptions {
                id: id.clone(),
                local_only: false,
            })
            .await?;

        let i = self.info(InfoOptions{ service: ServiceIdentifier::id(id.clone()) }).await?;

        Ok((h, i))
    }

    /// List known IoT services
    pub async fn list(&mut self, _options: ListOptions) -> Result<Vec<IotService>, IotError> {
        let req = rpc::service::ListOptions {
            application_id: Some(IOT_APP_ID),
        };

        let services = self.client.list(req).await?;

        debug!("Received service list: {:?}", services);
        
        let mut iot_services = Vec::with_capacity(services.len());

        for i in &services {
            // Fetch page signature from service info
            let page_sig = match &i.primary_page {
                Some(p) => p,
                None => {
                    warn!("No primary page signature for service: {}", i.id);
                    continue;
                }
            };

            // Fetch page by signature
            let p = self.client.page(FetchOptions{
                service: i.id.clone().into(),
                page_sig: page_sig.clone() }).await?;

            // Build IoT service object
            let iot_svc = match IotService::decode_page(i, p) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to decode page {} for service {}", page_sig, i.id);
                    continue;
                },
            };

            iot_services.push(iot_svc);
        }
        Ok(iot_services)
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
    pub async fn info(&mut self, options: InfoOptions) -> Result<IotService, IotError> {
        // Fetch service info object
        let (h, i) = self.client.info(options).await?;

        // Check we have a primary page object
        let page_sig = match &i.primary_page {
            Some(s) => s,
            None => return Err(IotError::Client(Error::NoPageFound)),
        };

        // Lookup page object
        let p = self.client.page(FetchOptions{service: h.into(), page_sig: page_sig.clone() }).await?;

        // TODO: Decrypt if possible / required
        //i.body.decrypt(i.secret_key.as_ref()).unwrap();

        // Coerce object into IotService
        IotService::decode_page(&i, p)
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
    pub async fn query(
        &mut self,
        options: QueryOptions,
    ) -> Result<(IotService, Vec<IotData>), IotError> {
        debug!("Querying for data: {:?}", options);

        let iot_info = self
            .info(InfoOptions {
                service: options.service.clone(),
            })
            .await?;

        debug!("info: {:?}", iot_info);

        let mut data_info = self.client.data(options).await?;

        let iot_data = data_info.drain(..).filter_map(|v| {
            IotData::decode(v, None).ok()
        }).collect();

        Ok((iot_info, iot_data))
    }

    /// Subscribe to data from an IoT service
    pub async fn subscribe(
        &mut self,
        options: rpc::SubscribeOptions,
    ) -> Result<impl Stream<Item = ()>, ClientError> {
        debug!("Subscribe to service: {:?}", options);

        let resp = self.client.subscribe(options).await?;

        // TODO: decode endpoint info here
        Ok(resp.map(|_d| ()))
    }

    pub fn generate() -> Result<(Id, Keys), ClientError> {
        use dsf_core::crypto;
        
        let (pub_key, pri_key) = crypto::new_pk()?;
        let id = crypto::hash(&pub_key)?;
        let sec_key = crypto::new_sk()?;

        let keys = Keys{
            pub_key, 
            pri_key: Some(pri_key), 
            sec_key: Some(sec_key), 
            sym_keys: None};

        Ok((id, keys))
    }

    pub fn encode(opts: &EncodeOptions) -> Result<(), IotError> {

        debug!("Encoding endpoints: {:?}", opts.create.endpoints);
        // Encode body
        let mut body = vec![0u8; 1024];
        let n = IotService::encode_body(&opts.create.endpoints, &mut body)?;

        // TODO: Encode meta

        debug!("Building service");

        // Create service object
        let mut s = ServiceBuilder::default()
            .body(Body::Cleartext((&body[..n]).to_vec()))
            .build()?;

        debug!("Generating service page");

        // Encode generate service page
        let mut buff = vec![0u8; 1024];
        let (n, p) = s.publish_primary(&mut buff)?;

        info!("Created page: {:?}", p);

        #[cfg(feature="alloc")]
        info!("Data: {:?}", buff.hex_dump());

        if let Some(f) = &opts.file {
            info!("Writing to file: {}", f);

            std::fs::write(f, &buff[..n])?;
        }

        Ok(())
    }

    pub fn decode(opts: DecodeOptions) -> Result<(), Error> {
        todo!()
    }
}
