use core::convert::TryInto;

use futures::prelude::*;
use log::{debug, error, warn};

use encdec::EncodeExt;

#[cfg(feature = "alloc")]
use pretty_hex::*;

pub use dsf_client::{prelude::*, Config, Error};

pub use dsf_rpc::ServiceIdentifier;
use dsf_rpc::{self as rpc, DataInfo, PublishInfo, ServiceInfo};

use dsf_core::{
    api::{Application, ServiceHandle},
    crypto::{Crypto, Hash},
    prelude::*,
    types::ServiceKind,
};
use rpc::NsRegisterInfo;

use crate::error::IotError;
use crate::prelude::{EpData, EpDescriptor, EpFlags};
use crate::IoT;

pub mod options;
pub use options::*;

/// IotClient wraps a `dsf_client::Client` and provides interfaces to interact with DSF-IoT services
/// TODO: one day this could be an extension trait?
pub struct IotClient {
    client: Client,
}

impl IotClient {
    /// Create a new DSF-IoT client using the provided path
    pub async fn new(config: &Config) -> Result<Self, IotError> {
        let client = Client::new(config).await?;

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

    /// Discover local IoT services
    pub async fn discover(
        &mut self,
        opts: DiscoverOptions,
    ) -> Result<Vec<(ServiceInfo, DataInfo<Vec<EpDescriptor>>)>, IotError> {
        // Build discovery filters
        let eps: Vec<_> = opts
            .endpoints
            .iter()
            .map(|k| EpDescriptor::new(*k, EpFlags::empty()))
            .collect();
        let (body, _) = eps.encode_vec()?;

        // Issue discovery request
        let locate_info = self
            .client
            .discover(rpc::DiscoverOptions {
                application_id: 1,
                body: Some(body),
                filters: opts.options.to_vec(),
            })
            .await?;

        // Load information for discovered services
        let mut services = vec![];
        for i in &locate_info {
            let (s, d) = self
                .info(InfoOptions {
                    service: ServiceIdentifier::id(i.id.clone()),
                })
                .await?;

            services.push((s, d));
        }

        Ok(services)
    }

    /// Register an existing service in the database
    pub async fn register(
        &mut self,
        options: RegisterOptions,
    ) -> Result<dsf_rpc::service::RegisterInfo, IotError> {
        let r = self.client.register(options).await?;
        Ok(r)
    }

    /// Search for an existing IoT service in the database
    pub async fn search(
        &mut self,
        id: &Id,
    ) -> Result<(ServiceHandle, ServiceInfo, DataInfo<Vec<EpDescriptor>>), IotError> {
        let (h, _i) = self
            .client
            .locate(LocateOptions {
                id: id.clone(),
                local_only: false,
            })
            .await?;

        let i = self
            .info(InfoOptions {
                service: ServiceIdentifier::id(id.clone()),
            })
            .await?;

        Ok((h, i.0, i.1))
    }

    /// List known IoT services
    pub async fn list(
        &mut self,
        _options: ListOptions,
    ) -> Result<Vec<(ServiceInfo, DataInfo<Vec<EpDescriptor>>)>, IotError> {
        let req = rpc::service::ListOptions {
            application_id: Some(IoT::APPLICATION_ID),
            kind: Some(ServiceKind::Generic),
        };

        let services = self.client.list(req).await?;

        debug!("Received service list: {:?}", services);

        let mut iot_services = Vec::with_capacity(services.len());

        for service_info in &services {
            // Fetch page signature from service info
            let page_sig = match &service_info.primary_page {
                Some(p) => p,
                None => {
                    warn!("No primary page signature for service: {}", service_info.id);
                    continue;
                }
            };

            // Fetch page by signature
            let page_info = self
                .client
                .object(rpc::FetchOptions {
                    service: service_info.id.clone().into(),
                    page_sig: page_sig.clone(),
                })
                .await?;

            // Parse page info using iot application body
            let page_info = match page_info.convert::<Vec<EpDescriptor>>() {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "Failed to decode endpoints for service {}: {:?}",
                        service_info.id, e
                    );
                    continue;
                }
            };

            iot_services.push((service_info.clone(), page_info));
        }

        Ok(iot_services)
    }

    /// Fetch service information
    pub async fn info(
        &mut self,
        options: InfoOptions,
    ) -> Result<(ServiceInfo, DataInfo<Vec<EpDescriptor>>), IotError> {
        // Fetch service info object
        let (h, service_info) = self.client.info(options).await?;

        // Check we have a primary page object
        let page_sig = match &service_info.primary_page {
            Some(s) => s,
            None => return Err(IotError::Client(Error::NoPageFound)),
        };

        // Lookup page object
        let page_info = self
            .client
            .object(rpc::FetchOptions {
                service: h.into(),
                page_sig: page_sig.clone(),
            })
            .await?;

        // Parse page info using iot application body
        let page_info = match page_info.convert::<Vec<EpDescriptor>>() {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "Failed to decode endpoints for service {}: {:?}",
                    service_info.id, e
                );
                return Err(e.into());
            }
        };

        Ok((service_info, page_info))
    }

    /// Publish raw data using an existing IoT service
    pub async fn publish_raw(
        &mut self,
        service: ServiceIdentifier,
        kind: u8,
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

    /// Query for data from an IoT service
    pub async fn query(
        &mut self,
        options: QueryOptions,
    ) -> Result<
        (
            ServiceInfo,
            DataInfo<Vec<EpDescriptor>>,
            Vec<DataInfo<Vec<EpData>>>,
        ),
        IotError,
    > {
        debug!("Querying for data: {:?}", options);

        let iot_info = self
            .info(InfoOptions {
                service: options.service.clone(),
            })
            .await?;

        debug!("info: {:?}", iot_info);

        let mut data_info = self.client.data(options).await?;

        // Filter and convert data objects
        let iot_data = data_info
            .drain(..)
            .filter_map(|i| {
                if i.kind.is_page() {
                    return None;
                }

                i.convert::<Vec<EpData>>().ok()
            })
            .collect();

        Ok((iot_info.0, iot_info.1, iot_data))
    }

    /// Register an IoT service with the specified nameservice
    pub async fn ns_register(
        &mut self,
        opts: NsRegisterOptions,
    ) -> Result<NsRegisterInfo, IotError> {
        debug!("Registering service: {:?}", opts.target);

        // Fetch information for services to be registered
        let (_s, d) = self
            .info(InfoOptions {
                service: opts.target.clone().into(),
            })
            .await?;

        // Generate hashes for endpoints
        let mut hashes = vec![];
        if let MaybeEncrypted::Cleartext(eps) = d.body {
            for e in eps {
                let v = u16::from(&e.kind);
                let h = Crypto::hash(&v.to_le_bytes()).unwrap();
                hashes.push(h);
            }
        }

        let options: Vec<_> = d
            .public_options
            .iter()
            .filter(|f| f.filterable())
            .map(|o| o.clone())
            .collect();

        debug!("Using hashes: {:?}, options: {:?}", hashes, options);

        debug!("Registering service");

        let r = self
            .client
            .ns_register(rpc::NsRegisterOptions {
                ns: opts.ns,
                target: opts.target.clone(),
                name: Some(opts.name),
                options,
                hashes,
            })
            .await?;

        Ok(r)
    }

    pub async fn ns_search(
        &mut self,
        opts: NsSearchOptions,
    ) -> Result<Vec<(ServiceInfo, DataInfo<Vec<EpDescriptor>>)>, IotError> {
        debug!("Searching via nameservice: {:?}", opts.ns);

        // Resolve endpoint kind to hash if required
        let hash = match opts.endpoint {
            Some(e) => {
                let v = u16::from(&e);
                Some(Crypto::hash(&v.to_le_bytes()).unwrap())
            }
            None => None,
        };

        // Perform search with nameservice
        let locate_info = self
            .client
            .ns_search(rpc::NsSearchOptions {
                ns: opts.ns,
                name: opts.name.clone(),
                options: opts.options.clone(),
                hash: hash,
            })
            .await?;

        // Load information for discovered services
        let mut services = vec![];
        for i in &locate_info {
            let (s, d) = self
                .info(InfoOptions {
                    service: ServiceIdentifier::id(i.id.clone()),
                })
                .await?;

            services.push((s, d));
        }

        Ok(services)
    }

    pub fn generate() -> Result<(Id, Keys), ClientError> {
        use dsf_core::crypto::{Hash as _, PubKey as _, SecKey as _};

        let (pub_key, pri_key) = Crypto::new_pk()?;
        let id = Crypto::hash(&pub_key)?;
        let sec_key = Crypto::new_sk()?;

        let keys = Keys {
            pub_key: Some(pub_key),
            pri_key: Some(pri_key),
            sec_key: Some(sec_key),
            sym_keys: None,
        };

        Ok((id.into(), keys))
    }
}

#[cfg(test)]
mod test {}
