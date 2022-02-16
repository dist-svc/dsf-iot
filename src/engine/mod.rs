use core::fmt::{Debug};
use core::convert::TryFrom;

use dsf_core::types::ImmutableData;
use dsf_core::wire::Container;
use log::{trace, debug, info, warn, error};

use dsf_core::{prelude::*, options::Options, net::Status};
use dsf_core::base::{Parse, DataBody};

use crate::{IOT_APP_ID};
use crate::endpoint::{Descriptor};

mod store;
pub use store::*;

mod comms;
pub use comms::*;


// Trying to build an abstraction over IP, LPWAN, (UNIX to daemon?)

pub struct Engine<'a, C: Comms, D: AsRef<[Descriptor]> = Vec<Descriptor>, S: Store = MemoryStore, const N: usize = 512> {
    descriptors: D,
    svc: Service,

    pri: Signature,
    req_id: u16,

    comms: C,
    store: S,

    on_rx: Option<&'a mut dyn FnMut(&Container)>,
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature="thiserror", derive(thiserror::Error))]
pub enum EngineError<CommsError: Debug, StoreError: Debug> {

    #[cfg_attr(feature="thiserror", error("core: {0:?}"))]
    Core(dsf_core::error::Error),
    
    #[cfg_attr(feature="thiserror", error("comms: {0:?}"))]
    Comms(CommsError),

    #[cfg_attr(feature="thiserror", error("store: {0:?}"))]
    Store(StoreError),

    #[cfg_attr(feature="thiserror", error("unhandled"))]
    Unhandled,

    #[cfg_attr(feature="thiserror", error("unsupported"))]
    Unsupported,
}

#[derive(Debug, PartialEq)]
pub enum EngineEvent {
    None,
    Discover(Id),
    SubscribeFrom(Id),
    UnsubscribeFrom(Id),
    SubscribedTo(Id),
    UnsubscribedTo(Id),
    ReceivedData(Id),
}

#[derive(Debug, PartialEq)]
enum EngineResponse {
    None,
    Net(NetResponseBody),
    Page(Container),
}

impl From<NetResponseBody> for EngineResponse {
    fn from(r: NetResponseBody) -> Self {
        Self::Net(r)
    }
}

impl From<Container> for EngineResponse {
    fn from(p: Container) -> Self {
        Self::Page(p)
    }
}

pub trait Filter<V> {
    fn matches(&self, v: V) -> bool;
}

impl <V: PartialEq> Filter<V> for V {
    fn matches(&self, v: V) -> bool {
        self == &v
    }
}

impl <V: PartialEq> Filter<V> for &[V] {
    fn matches(&self, v: V) -> bool {
        self.contains(&v)
    }
}

impl <'a, A, C, D, S, const N: usize> Engine<'a, C, D, S, N> 
where
    A: Clone + Debug, 
    C: Comms<Address=A>, 
    D: AsRef<[Descriptor]>, 
    S: Store<Address=A>,
{

    pub fn new(descriptors: D, comms: C, mut store: S) -> Result<Self, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        let mut sb = ServiceBuilder::generic();

        // Start assembling the service
        sb = sb.application_id(IOT_APP_ID);

        // Attempt to load existing keys
        if let Some(k) = store.get_ident().map_err(EngineError::Store)? {
            debug!("Using existing keys: {:?}", k);
            sb = sb.keys(k);
        }

        // Attempt to load last sig for continuation
        // TODO: should this fetch the index too?
        if let Some(s) = store.get_last().map_err(EngineError::Store)? {
            debug!("Using last info: {:?}", s);
            sb = sb.last_signature(s.sig);
            sb = sb.last_page(s.page_index);
        }

        // TODO: fetch existing page if available?

        // Create service
        let mut svc = sb.build().map_err(EngineError::Core)?;

        // TODO: do not regenerate page if not required

        // Generate initial page
        let mut page_buff = [0u8; N];
        let (_n, p) = svc.publish_primary(Default::default(), &mut page_buff)
            .map_err(EngineError::Core)?;
        
        let sig = p.signature();

        trace!("Generated new page: {:?} sig: {}", p, sig);

        // Update last signature in store
        let info = ObjectInfo{page_index: p.header().index(), block_index: 0, sig: sig.clone()};
        store.set_last(&info)
            .map_err(EngineError::Store)?;


        // Store page if possible
        store.store_page(&sig, &p)
            .map_err(EngineError::Store)?;

        // TODO: setup forward to subscribers?

        // Return object
        Ok(Self{ descriptors, svc, pri: sig, req_id: 0, comms, store, on_rx: None })
    }

    pub fn id(&self) -> Id {
        self.svc.id()
    }

    fn next_req_id(&mut self) -> u16 {
        self.req_id = self.req_id.wrapping_add(1);
        self.req_id
    }

    pub fn set_handler(&mut self, on_rx: &'a mut dyn FnMut(&Container)) {
        self.on_rx = Some(on_rx);
    }

    /// Discover local services
    pub fn discover(&mut self, body: &[u8], opts: &[Options]) -> Result<u16, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        debug!("Generating local discovery request");

        // Generate discovery request
        let req_id = self.next_req_id();
        let req_body = NetRequestBody::Discover(body.to_vec(), opts.to_vec());
        let mut req = NetRequest::new(self.id(), req_id, req_body, Flags::PUB_KEY_REQUEST);
        req.common.public_key = Some(self.svc.public_key());


        debug!("Broadcasting discovery request: {:?}", req);

        // Sending discovery request
        let c = self.svc.encode_request_buff(&req, &Default::default())
                .map_err(EngineError::Core)?;

        trace!("Container: {:?}", c);

        self.comms.broadcast(c.raw()).map_err(EngineError::Comms)?;

        Ok(req_id)
    }

    /// Publish service data
    pub fn publish<B: DataBody>(&mut self, body: B, opts: &[Options]) -> Result<Signature, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        
        // TODO: Fetch last signature / associated primary page

        // Setup page options for encoding
        let page_opts = DataOptions::<B>{
            body: Some(body),
            public_options: opts,
            ..Default::default()
        };

        // Publish data to buffer
        let (_n, p) = self.svc.publish_data_buff(page_opts)
            .map_err(EngineError::Core)?;

        let data = p.raw();
        let sig = p.signature();

        let info = ObjectInfo{
            page_index: self.svc.version(), 
            block_index: p.header().index(),
            sig: sig.clone(),
        };

        debug!("Publishing object: {:02x?}", p);

        // Update last sig
        self.store.set_last(&info)
            .map_err(EngineError::Store)?;

        // Write to store
        self.store.store_page(&sig, &p)
            .map_err(EngineError::Store)?;

        // Send updated page to subscribers
        for (id, peer) in self.store.peers() {
            match (&peer.subscriber, &peer.addr) {
                (true, Some(addr)) => {
                    debug!("Forwarding data to: {} ({:?})", id, addr);
                    self.comms.send(addr, data).map_err(EngineError::Comms)?;
                },
                _ => (),
            }
        }

        Ok(sig)
    }

    /// Subscribe to the specified service, optionally using the provided address
    pub fn subscribe(&mut self, id: Id, addr: A) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        // TODO: for delegation peers != services, do we need to store separate objects for this?

        debug!("Attempting to subscribe to: {} at: {:?}", id, addr);

        // Generate request ID and update peer
        let req_id = self.next_req_id();

        // Update subscription
        self.store.update_peer(&id, |p| {
            // TODO: include who this is via
            p.subscribed = SubscribeState::Subscribing(req_id);
        }).map_err(EngineError::Store)?;

        // Send subscribe request
        // TODO: how to separate target -service- from target -peer-
        let req = NetRequestBody::Subscribe(id);
        self.request(&addr, req_id, req)?;

        debug!("Subscribe TX done (req_id: {})", req_id);

        Ok(())
    }

    /// Update internal state
    pub fn update(&mut self) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {

        // TODO: regenerate primary page if required

        // TODO: walk subscribers and expire if required

        // TODO: walk subscriptions and re-subscribe if required

        todo!()
    }

    /// Send a request
    fn request(&mut self, addr: &A, req_id: u16, data: NetRequestBody) -> Result<(), EngineError<<C as Comms>::Error, <S as    Store>::Error>> {
        let mut flags = Flags::empty();

        // TODO: set pub_key request flag for unknown peers

        let req = NetRequest::new(self.svc.id(), req_id, data, flags);

        // TODO: include peer keys here if available
        let c = self.svc.encode_request_buff(&req, &Default::default())
                .map_err(EngineError::Core)?;

        self.comms.send(&addr, c.raw()).map_err(EngineError::Comms)?;

        Ok(())
    }

    /// Handle received data
    pub fn handle(&mut self, from: A, data: &[u8]) -> Result<EngineEvent, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        debug!("Received {} bytes from {:?}", data.len(), from);

        // Parse base object
        let base = match Container::parse(data, &self.store) {
            Ok(v) => (v),
            Err(e) => {
                error!("DSF parsing error: {:?}", e);
                return Err(EngineError::Core(e))
            }
        };

        debug!("Received object: {:02x?}", base);

        let req_id = base.header().index();

        // Convert and handle messages
        let (resp, evt) = match NetMessage::convert(base.clone(), &self.store) {
            Ok(NetMessage::Request(req)) => self.handle_req(&from, req)?,
            Ok(NetMessage::Response(resp)) => self.handle_resp(&from, resp)?,
            _ if base.header().kind().is_page() => self.handle_page(&from, base)?,
            _ if base.header().kind().is_data() => self.handle_page(&from, base)?,
            _ => {
                error!("Unhandled object type");
                return Err(EngineError::Unhandled)
            }
        };

        // Send responses
        match resp {
            EngineResponse::Net(net) => {
                debug!("Sending response {:?} (id: {}) to: {:?}", net, req_id, from);
                let r = NetResponse::new(self.svc.id(), req_id, net, Default::default());

                // TODO: pass peer keys here
                let c = self.svc.encode_response_buff(&r, &Default::default())
                    .map_err(EngineError::Core)?;
                
                self.comms.send(&from, c.raw()).map_err(EngineError::Comms)?;
            },
            EngineResponse::Page(p) => {
                debug!("Sending page {:?} to: {:?}", p, from);
                // TODO: ensure page is valid prior to sending?
                self.comms.send(&from, p.raw()).map_err(EngineError::Comms)?;
            },
            EngineResponse::None => (),
        }

        Ok(evt)
    }



    fn handle_req(&mut self, from: &A, req: NetRequest) -> Result<(EngineResponse, EngineEvent), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        use NetRequestBody::*;

        debug!("Received request: {:?} from: {} ({:?})", req, req.common.from, from);

        if let Some(pub_key) = req.common.public_key {
            debug!("Update peer: {:?}", from);
            self.store.update_peer(&req.common.from, |p| {
                p.keys.pub_key = Some(pub_key.clone());
                p.addr = Some(from.clone());
            }).map_err(EngineError::Store)?;
        }

        let mut evt = EngineEvent::None;

        // Handle request messages
        let resp: EngineResponse = match &req.data {
            Hello | Ping => NetResponseBody::Status(Status::Ok).into(),
            Discover(body, options) => {
                debug!("Received discovery from {} ({:?})", req.common.from, from);

                let mut matches = false;

                // Iterate through matching endpoints
                for e in Descriptor::parse_iter(body).filter_map(|d| d.ok() ) {
                    if self.descriptors.as_ref().contains(&e) {
                        debug!("Filter match on endpoint: {:?}", e);
                        matches = true;
                        break;
                    }
                }

                // Iterate through matching options
                for o in options {
                    if self.svc.public_options().contains(o) {
                        debug!("Filter match on option: {:?}", o);
                        matches = true;
                        break;
                    }
                }

                if !matches {
                    debug!("No match for discovery message");
                    EngineResponse::None
                    
                } else {
                    // Respond with page if filters pass
                    match self.store.fetch_page(&self.pri) {
                        Ok(Some(p)) => p.into(),
                        _ => EngineResponse::None,
                    }
                }
            },
            Query(id) if id == &self.svc.id() => {
                debug!("Sending service information to {} ({:?})", req.common.from, from);

                if let Some(p) = self.store.fetch_page(&self.pri)
                        .map_err(EngineError::Store)? {
                    p.into()
                } else {
                    NetResponseBody::Status(Status::InvalidRequest).into()
                }
            },
            Subscribe(id) if id == &self.svc.id() => {
                debug!("Adding {} ({:?}) as a subscriber", req.common.from, from);

                self.store.update_peer(&req.common.from, |p| {
                    p.subscriber = true;
                    p.addr = Some(from.clone());
                }).map_err(EngineError::Store)?;

                evt = EngineEvent::SubscribeFrom(req.common.from.clone());

                NetResponseBody::Status(Status::Ok).into()
            },
            Unsubscribe(id) if id == &self.svc.id() => {
                debug!("Removing {} ({:?}) as a subscriber", req.common.from, from);

                self.store.update_peer(&req.common.from, |p| {
                    p.subscriber = false;
                }).map_err(EngineError::Store)?;

                evt = EngineEvent::UnsubscribeFrom(req.common.from.clone());

                NetResponseBody::Status(Status::Ok).into()
            },
            Subscribe(_id) | Unsubscribe(_id) => {
                NetResponseBody::Status(Status::InvalidRequest).into()
            },
            //PushData(id, pages) => ()
            _ => NetResponseBody::Status(Status::InvalidRequest).into()
        };

        Ok((resp, evt))
    }

    fn handle_resp(&mut self, from: &A, resp: NetResponse) -> Result<(EngineResponse, EngineEvent), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        //use NetResponseBody::*;

        debug!("Received response: {:?} from: {:?}", resp, from);

        let req_id = resp.common.id;
        let mut evt = EngineEvent::None;

        // Find matching peer for response
        let peer = match self.store.get_peer(&resp.common.from).map_err(EngineError::Store)? {
            Some(p) => p,
            None => {
                panic!();
            },
        };

        // Handle response messages
        match (&peer.subscribed, &resp.data) {
            // Subscribe responses
            (SubscribeState::Subscribing(id), NetResponseBody::Status(st)) if req_id == *id => {
                if *st == Status::Ok {
                    info!("Subscribe ok for {} ({:?})", resp.common.from, from);

                    let p = self.store.update_peer(&resp.common.from, |p| {
                        p.subscribed = SubscribeState::Subscribed;
                    }).map_err(EngineError::Store)?;
                    
                    evt = EngineEvent::SubscribedTo(resp.common.from.clone());
                    p

                } else {
                    info!("Subscribe failed for {} ({:?})", resp.common.from, from);

                }
            },
            // Unsubscribe response
            (SubscribeState::Unsubscribing(id), NetResponseBody::Status(st)) if req_id == *id => {
                if *st == Status::Ok {
                    info!("Unsubscribe ok for {} ({:?})", resp.common.from, from);

                    let p = self.store.update_peer(&resp.common.from, |p| {
                        p.subscribed = SubscribeState::None;
                    }).map_err(EngineError::Store)?;

                    evt = EngineEvent::UnsubscribedTo(resp.common.from.clone());
                    p

                } else {
                    info!("Unsubscribe failed for {} ({:?})", resp.common.from, from);

                }
            },
            // TODO: what other responses are important?
            //NoResult => (),
            //PullData(_, _) => (),
            (_, NetResponseBody::Status(status)) => {
                debug!("Received status: {:?} for peer: {:?}", status, peer);
            }
            _ => todo!(),
        };


        Ok((EngineResponse::None, evt))
    }

    fn handle_page<T: ImmutableData>(&mut self, from: &A, p: Container<T>) -> Result<(EngineResponse, EngineEvent), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        debug!("Received page: {:?} from: {:?}", p, from);

        let mut evt = EngineEvent::None;

        // Find matching peer for rx'd page
        let peer = match self.store.get_peer(&p.id()).map_err(EngineError::Store)? {
            Some(p) => p,
            None => {
                warn!("No peer for page from id: {}", p.id());

                self.store.update_peer(&p.id(), |peer| {
                    if let Ok(PageInfo::Primary(pri)) = p.info() {
                        peer.keys.pub_key = Some(pri.pub_key.clone());
                    }
                }).map_err(EngineError::Store)?;

                return Ok((NetResponseBody::Status(Status::Ok).into(), evt));
            },
        };

        // Check for subscription
        if !peer.subscribed() {
            warn!("Not subscribed to peer: {}", p.id());
            return Ok((NetResponseBody::Status(Status::InvalidRequest).into(), evt));
        }

        // Emit rx event
        evt = EngineEvent::ReceivedData(p.id().clone());

        // Call receive handler
        if let Some(on_rx) = self.on_rx.as_mut() {
            (on_rx)(&p.to_owned());
        }

        // Respond with OK
        Ok((NetResponseBody::Status(Status::Ok).into(), evt))
    }
}


#[cfg(test)]
mod test {

    //use dsf_core::prelude::*;
    use dsf_core::net::Status;
    use dsf_core::options::Metadata;
    
    use crate::prelude::*;
    use crate::endpoint::{self as ep};
    use crate::service::{IotData};

    use super::*;

    use super::comms::MockComms;

    // Setup an engine instance for testing
    fn setup<'a>() -> (Service, Engine<'a, MockComms, Vec<Descriptor>, MemoryStore<u8>>) {
        // Setup debug logging
        let _ = simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, simplelog::Config::default());

        // Create peer for sending requests
        let p = ServiceBuilder::generic().build().unwrap();

        // Setup memory store with pre-filled peer keys
        let mut s = MemoryStore::<u8>::new();
        s.update(&p.id(), |k| *k = p.keys() );

        // Setup descriptors
        let descriptors = vec![
            Descriptor::new(ep::Kind::Temperature, ep::Flags::R, vec![]),
            Descriptor::new(ep::Kind::Pressure, ep::Flags::R, vec![]),
            Descriptor::new(ep::Kind::Humidity, ep::Flags::R, vec![]),
        ];

        // Setup engine with default service
        let e = Engine::new(descriptors, MockComms::default(), s)
                .expect("Failed to create engine");

        (p, e)
    }


    #[test]
    fn test_handle_reqs() {
        // Create peer for sending requests
        let (p, mut e) = setup();
        let from = 1;

        let tests = [
            (NetRequestBody::Hello,                     NetResponseBody::Status(Status::Ok)),
            (NetRequestBody::Ping,                      NetResponseBody::Status(Status::Ok)),
            //(NetRequestBody::Query(e.svc.id()),         NetResponseBody::Status(Status::Ok)),
            //(NetRequestBody::Subscribe(e.svc.id()),     NetResponseBody::Status(Status::Ok)),
            //(NetRequestBody::Unsubscribe(e.svc.id()),   NetResponseBody::Status(Status::Ok)),
        ];

        for t in &tests {
            // Generate full request object
            let req = NetRequest::new(p.id(), 1, t.0.clone(), Default::default());

            // Pass to engine
            let (resp, _evt) = e.handle_req(&from, req.clone())
                .expect("Failed to handle message");

            // Check response
            assert_eq!(resp, t.1.clone().into(),
                "Unexpected response for request: {:#?}", req);
        }
    }

    #[test]
    fn test_handle_subscribe() {
        let (p, mut e) = setup();
        let from = 1;

        // Build subscribe request and execute
        let req = NetRequest::new(p.id(), 1, NetRequestBody::Subscribe(e.svc.id()), Default::default());
        let (resp, _evt) = e.handle_req(&from, req).expect("Failed to handle message");

        // Check response
        assert_eq!(resp, NetResponseBody::Status(Status::Ok).into());

        // Check subscriber state
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscriber ), Some(true));

        // TODO: expiry?
    }

    #[test]
    fn test_handle_unsubscribe() {
        let (p, mut e) = setup();
        let from = 1;

        e.store.update_peer(&p.id(), |p| p.subscriber = true ).unwrap();

        // Build subscribe request and execute
        let req = NetRequest::new(p.id(), 1, NetRequestBody::Unsubscribe(e.svc.id()), Default::default());
        let (resp, _evt) = e.handle_req(&from, req).expect("Failed to handle message");

        // Check response
        assert_eq!(resp, NetResponseBody::Status(Status::Ok).into());

        // Check subscriber state
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscriber ), Some(false));
    }

    #[test]
    fn test_handle_discover() {
        let (p, mut e) = setup();
        let from = 1;

        // Build net request and execute
        let ep_filter: &[Descriptor] = &[Descriptor::new(ep::Kind::Temperature, ep::Flags::R, vec![])];
        let (body, n) = ep_filter.encode_buff::<128>().unwrap();
        let req = NetRequest::new(p.id(), 1, NetRequestBody::Discover((&body[..n]).to_vec(), vec![]), Default::default());
        let (resp, _evt) = e.handle_req(&from, req).expect("Failed to handle message");

        // Check response
        assert_eq!(resp, e.store.fetch_page(&e.pri).unwrap().unwrap().into());
    }


    #[test]
    fn test_publish() {
        let (p, mut e) = setup();
        let from = 1;

        // Setup peer as subscriber
        e.store.update_peer(&p.id(), |p| {
            p.subscriber = true;
            p.addr = Some(from);
        }).unwrap();

        // Build object for publishing
        let endpoint_data = IotData::<stor::Const<0>, _>::new([
            EpData::new(27.3.into(), []),
            EpData::new(1016.2.into(), []),
            EpData::new(59.6.into(), []),
        ]);

        let mut data_buff = [0u8; 128];
        let n = endpoint_data.encode(&mut data_buff).unwrap();

        // Call publish operation
        e.publish(&data_buff[..n], &[])
            .expect("Publishing error");

        // Check outgoing data
        let d = e.comms.tx.pop().unwrap();
        assert_eq!(d.0, from);

        // Parse out page
        let _b = Container::parse(&d.1, &e.svc.keys()).expect("Failed to parse object");

        // TODO: translate back to IoT data and check

    }

    #[test]
    fn test_subscribe() {
        let (mut p, mut e) = setup();
        let from = 1;

        // Setup peer as subscriber
        e.store.update_peer(&p.id(), |p| {
            p.addr = Some(from);
        }).unwrap();

        // Call publish operation
        e.subscribe(p.id(), from)
            .expect("Subscribing error");

        // Check peer state
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscribed ), Some(SubscribeState::Subscribing(e.req_id)));

        // Check outgoing data
        let d = e.comms.tx.pop().expect("No outgoing data found");
        assert_eq!(d.0, from, "outgoing address mismatch");


        // Parse out page and convert back to message
        let b = Container::parse(&d.1, &e.svc.keys()).expect("Failed to parse object");
        let m = NetMessage::convert(b, &e.store).expect("Failed to convert message");

        let expected = NetRequest::new(e.svc.id(), e.req_id, 
                NetRequestBody::Subscribe(p.id()), Default::default());

        assert_eq!( m, NetMessage::Request(expected), "Request mismatch");


        // Respond with subscribe ok
        let mut buff = [0u8; 512];
        let (_n, sp) = p.publish_primary(Default::default(), &mut buff).unwrap();

        let resp = NetResponse::new(p.id(), e.req_id, NetResponseBody::Status(Status::Ok), Default::default());
        e.handle_resp(&from, resp).expect("Response handling failed");

        // Check peer state is now subscribed
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscribed ), Some(SubscribeState::Subscribed));


        // Test receiving data
        let mut new_page = None;
        let mut h = |page: &Container| new_page = Some(page.clone()) ;
        e.set_handler(&mut h);

        e.handle_page(&from, sp.to_owned()).expect("Failed to handle page");
        assert_eq!(Some(sp.to_owned()), new_page);
    }



}