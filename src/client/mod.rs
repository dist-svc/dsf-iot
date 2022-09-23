use core::convert::{TryInto};

use dsf_core::wire::Container;
use futures::prelude::*;
use log::{debug, info, warn};

use encdec::{Encode, Decode};

#[cfg(feature="alloc")]
use pretty_hex::*;

use dsf_client::prelude::*;
use dsf_rpc::{self as rpc, PublishInfo};

use dsf_core::api::{ServiceHandle, Application};
use dsf_core::prelude::*;
use dsf_core::types::DataKind;

pub use dsf_client::{Error, Options};
pub use dsf_rpc::ServiceIdentifier;
use rpc::{FetchOptions, DataInfo};

use crate::error::IotError;
use crate::prelude::{EpDescriptor, IotData};
use crate::{service::*, IoT};

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
            application_id: Some(IoT::APPLICATION_ID),
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
            let iot_svc = match IotService::decode_page(p, i.secret_key.as_ref()) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to decode page {} for service {}: {:?}", page_sig, i.id, e);
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
        IotService::decode_page(p, i.secret_key.as_ref())
    }

    /// Publish raw data using an existing IoT service
    pub async fn publish_raw(
        &mut self,
        service: ServiceIdentifier,
        kind: u16,
        data: &[u8],
    ) -> Result<PublishInfo, IotError> {
        let p = dsf_rpc::data::PublishOptions {
            service,
            // TODO: reintroduce kinds
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
    ) -> Result<(IotService, Vec<(DataInfo, IotData)>), IotError> {
        debug!("Querying for data: {:?}", options);

        let iot_info = self
            .info(InfoOptions {
                service: options.service.clone(),
            })
            .await?;

        debug!("info: {:?}", iot_info);

        let mut data_info = self.client.data(options).await?;

        let iot_data = data_info.drain(..).filter_map(|i| {
            // TODO: handle keys correctly / usefully.
            let body = match &i.body {
                MaybeEncrypted::Cleartext(b) => b,
                _ => return None,
            };

            IotData::<8>::decode(body).map(|v| (i, v.0)).ok()
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
        use dsf_core::crypto::{Crypto, PubKey as _, SecKey as _, Hash as _};
        
        let (pub_key, pri_key) = Crypto::new_pk()?;
        let id = Crypto::hash(&pub_key)?;
        let sec_key = Crypto::new_sk()?;

        let keys = Keys{
            pub_key: Some(pub_key), 
            pri_key: Some(pri_key), 
            sec_key: Some(sec_key), 
            sym_keys: None};

        Ok((id.into(), keys))
    }

    pub fn encode(opts: &EncodeOptions) -> Result<(), IotError> {

        debug!("Encoding endpoints: {:?}", opts.create.endpoints);
        // Encode body
        let mut body = vec![0u8; 1024];
        let n = IotService::encode_body(&opts.create.endpoints, &mut body)?;

        // TODO: Encode meta

        debug!("Building service");

        // Create service object
        let mut sb = ServiceBuilder::default()
            .body(&body[..n]);

        // Inject private key if provided
        if let Some(pri_key) = &opts.keys.pri_key {
            sb = sb.private_key(pri_key.clone());
        }

        // Setup encryption
        if let Some(sec_key) = &opts.keys.sec_key {
            sb = sb.secret_key(sec_key.clone());
        } else if !opts.create.public {
            sb = sb.encrypt()
        }

        let mut s = sb.build()?;

        info!("New service (id: {} pri: {} sec: {:?}", 
                s.id(), s.private_key().unwrap(), s.secret_key());

        debug!("Generating service page");

        // Encode generate service page
        let mut buff = vec![0u8; 1024];
        let (n, p) = s.publish_primary(Default::default(), &mut buff)?;

        info!("Created page: {:?}", p);

        info!("Encoded {} bytes", n);

        #[cfg(feature="alloc")]
        info!("Data: {:?}", (&buff[..n]).hex_dump());

        if let Some(f) = &opts.file {
            info!("Writing to file: {}", f);

            std::fs::write(f, &buff[..n])?;
        }

        Ok(())
    }

    pub fn decode(opts: &DecodeOptions) -> Result<(), IotError> {
        
        debug!("Reading file: {}", opts.file);
        let buff = std::fs::read(&opts.file)?;

        debug!("Decoding page (keys: {:?})", opts.keys);
        let p = Container::decode_pages(&buff, &opts.keys)?;

        info!("Decoded pages: {:?}", p);

        debug!("Loading service");

        let _s = Service::<Vec<u8>>::load(&p[0])?;
        // TODO: display service info

        debug!("Loading IoT data");

        match p[0].encrypted() {
            false => {
                let eps = IotService::decode_body(p[0].body_raw())?;
                for i in 0..eps.len() {
                    println!("{}: {}", i, eps[i]);
                }
            },
            true => {
                warn!("Encrypted page body, unable to parse endpoints");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_encode_decode() {
        

    }

}