use core::fmt::Debug;
use core::convert::TryFrom;

use log::{debug, info, warn, error};

use dsf_core::{prelude::*, options::Options, net::Status};
use dsf_core::base::{Parse, Encode};

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

    on_rx: Option<&'a mut dyn FnMut(&Page)>,
}

#[derive(Debug, PartialEq)]
pub enum EngineError<CommsError: Debug, StoreError: Debug> {
    Core(dsf_core::error::Error),
    
    Comms(CommsError),

    Store(StoreError),

    Unhandled,

    Unsupported,
}

pub enum EngineEvent {

}

#[derive(Debug, PartialEq)]
enum EngineResponse {
    None,
    Net(NetResponseKind),
    Page(Page),
}

impl From<NetResponseKind> for EngineResponse {
    fn from(r: NetResponseKind) -> Self {
        Self::Net(r)
    }
}

impl From<Page> for EngineResponse {
    fn from(p: Page) -> Self {
        Self::Page(p)
    }
}

pub trait EngineBody: Filter<Self::Body> + Parse<Output=Self::Body> + Encode {
    type Body;
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

impl EngineBody for Vec<u8> {
    type Body = Vec<u8>;
}

pub struct ServiceOptions<B: EngineBody = Vec<u8>, P: AsRef<[Options]> = Vec<Options>, O: AsRef<[Options]> = Vec<Options>> {
    pub body: B,
    pub public_options: P,
    pub private_options: O,
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
        if let Some(k) = store.get_ident() {
            debug!("Using existing keys: {:?}", k);
            sb = sb.keys(k);
        }

        // Attempt to load last sig for continuation
        // TODO: should this fetch the index too?
        if let Some(s) = store.get_last_sig() {
            debug!("Using last sig: {}", s);
            sb = sb.last_signature(s);
        }

        // TODO: fetch existing page if available?

        // Create service
        let mut svc = sb.build().map_err(EngineError::Core)?;

        // TODO: do not regenerate page if not required

        // Generate initial page
        let mut page_buff = [0u8; N];
        let (_n, p) = svc.publish_primary(&mut page_buff)
            .map_err(EngineError::Core)?;
        
        let sig = p.signature().unwrap();

        debug!("Generated new page: {:?} sig: {}", p, sig);

        // Update last signature in store
        store.set_last_sig(&sig)
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

    pub fn set_handler(&mut self, on_rx: &'a mut dyn FnMut(&Page)) {
        self.on_rx = Some(on_rx);
    }

    /// Discover local services
    pub fn discover(&mut self, body: &[u8], opts: &[Options]) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        todo!()
    }

    /// Publish service data
    pub fn publish(&mut self, body: &[u8], opts: &[Options]) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        
        // TODO: Fetch last signature / associated primary page

        // Setup page options for encoding
        let page_opts = DataOptions {
            body: Body::Cleartext(body.to_vec()),
            public_options: opts,
            ..Default::default()
        };

        // Publish data to buffer
        let (n, page_buff, page) = self.svc.publish_data_buff::<512>(page_opts).map_err(EngineError::Core)?;

        let data = &page_buff[..n];
        let sig = page.signature().unwrap();

        // Update last sig
        self.store.set_last_sig(&sig)
            .map_err(EngineError::Store)?;

        // Write to store
        self.store.store_page(&sig, &page)
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

        Ok(())
    }

    /// Subscribe to the specified service, optionally using the provided address
    pub fn subscribe(&mut self, id: Id, addr: A) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        // TODO: for delegation peers != services, do we need to store separate objects for this?

        debug!("Attempting to subscribe to: {} at: {:?}", id, addr);

        // Generate request ID and update peer
        self.req_id = self.req_id.wrapping_add(1);
        let req_id = self.req_id;

        // Update subscription
        self.store.update_peer(&id, |p| {
            // TODO: include who this is via
            p.subscribed = SubscribeState::Subscribing(req_id);
        }).map_err(EngineError::Store)?;

        // Send subscribe request
        // TODO: how to separate target -service- from target -peer-
        let req = NetRequest::new(self.svc.id(), req_id, NetRequestKind::Subscribe(id), Flags::empty());
        let (n, buff) = self.svc.encode_message_buff::<_, 512>(req)
                .map_err(EngineError::Core)?;

        self.comms.send(&addr, &buff[..n]).map_err(EngineError::Comms)?;

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

    /// Handle received data
    pub fn handle(&mut self, from: A, data: &[u8]) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        debug!("Received {} bytes from {:?}", data.len(), from);

        // Parse base object
        let (base, _n) = match Base::parse(data, &self.store) {
            Ok(v) => (v),
            Err(e) => {
                error!("DSF parsing error: {:?}", e);
                return Err(EngineError::Core(e))
            }
        };

        // Convert and handle messages
        let resp = match (NetMessage::convert(base.clone(), &self.store), Page::try_from(base)) {
            (Ok(NetMessage::Request(req)), _) => self.handle_req(&from, req)?,
            (Ok(NetMessage::Response(resp)), _) => self.handle_resp(&from, resp)?,
            (_, Ok(p)) => self.handle_page(&from, p)?,
            _ => {
                error!("Unhandled object type");
                return Err(EngineError::Unhandled)
            }
        };

        // Send responses
        match resp {
            EngineResponse::Net(net) => {
                self.req_id = self.req_id.wrapping_add(1);
                let r = NetResponse::new(self.svc.id(), self.req_id, net, Flags::empty());

                let (n, buff) = self.svc.encode_message_buff::<_, 512>(NetMessage::Response(r))
                    .map_err(EngineError::Core)?;
                
                self.comms.send(&from, &buff[..n]).map_err(EngineError::Comms)?;
            },
            EngineResponse::Page(p) => {
                debug!("Sending page to: {:?}", from);
                if let Some(r) = p.raw() {
                    self.comms.send(&from, data).map_err(EngineError::Comms)?;
                } else {
                    // TODO: encode page here if required (shouldn't really ever occur?)
                    todo!()
                }
            },
            EngineResponse::None => (),
        }

        Ok(())
    }



    fn handle_req(&mut self, from: &A, req: NetRequest) -> Result<EngineResponse, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        use NetRequestKind::*;

        debug!("Received request: {:?} from: {} ({:?})", req, req.common.from, from);

        // Handle request messages
        let resp: EngineResponse = match &req.data {
            Hello | Ping => NetResponseKind::Status(Status::Ok).into(),
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
                    NetResponseKind::Status(Status::InvalidRequest).into()
                }
            },
            Subscribe(id) if id == &self.svc.id() => {
                debug!("Adding {} ({:?}) as a subscriber", req.common.from, from);

                self.store.update_peer(&req.common.from, |p| {
                    p.subscriber = true;
                    p.addr = Some(from.clone());
                }).map_err(EngineError::Store)?;

                NetResponseKind::Status(Status::Ok).into()
            },
            Unsubscribe(id) if id == &self.svc.id() => {
                debug!("Removing {} ({:?}) as a subscriber", req.common.from, from);

                self.store.update_peer(&req.common.from, |p| {
                    p.subscriber = false;
                }).map_err(EngineError::Store)?;

                NetResponseKind::Status(Status::Ok).into()
            },
            Subscribe(_id) | Unsubscribe(_id) => {
                NetResponseKind::Status(Status::InvalidRequest).into()
            },
            //PushData(id, pages) => ()
            _ => NetResponseKind::Status(Status::InvalidRequest).into()
        };

        Ok(resp)
    }

    fn handle_resp(&mut self, from: &A, resp: NetResponse) -> Result<EngineResponse, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        //use NetResponseKind::*;

        debug!("Received response: {:?} from: {:?}", resp, from);

        let req_id = resp.common.id;

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
            (SubscribeState::Subscribing(id), NetResponseKind::Status(st)) if req_id == *id => {
                if *st == Status::Ok {
                    info!("Subscribe ok for {} ({:?})", resp.common.from, from);

                    self.store.update_peer(&resp.common.from, |p| {
                        p.subscribed = SubscribeState::Subscribed;
                    }).map_err(EngineError::Store)?

                } else {
                    info!("Subscribe failed for {} ({:?})", resp.common.from, from);

                }
            },
            // Unsubscribe response
            (SubscribeState::Unsubscribing(id), NetResponseKind::Status(st)) if req_id == *id => {
                if *st == Status::Ok {
                    info!("Unsubscribe ok for {} ({:?})", resp.common.from, from);

                    self.store.update_peer(&resp.common.from, |p| {
                        p.subscribed = SubscribeState::None;
                    }).map_err(EngineError::Store)?

                } else {
                    info!("Unsubscribe failed for {} ({:?})", resp.common.from, from);

                }
            },
            // TODO: what other responses are important?
            //NoResult => (),
            //PullData(_, _) => (),
            _ => todo!(),
        };


        Ok(EngineResponse::None)
    }

    fn handle_page(&mut self, from: &A, p: Page) -> Result<EngineResponse, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        debug!("Received page: {:?} from: {:?}", p, from);

        // Find matching peer for rx'd page
        let peer = match self.store.get_peer(p.id()).map_err(EngineError::Store)? {
            Some(p) => p,
            None => {
                error!("no peer for id: {}", p.id());
                return Ok(NetResponseKind::Status(Status::InvalidRequest).into());
            },
        };

        // Check for subscription
        if !peer.subscribed() {
            warn!("Not subscribed to peer: {}", p.id());
            return Ok(NetResponseKind::Status(Status::InvalidRequest).into());
        }

        // Call receive handler
        if let Some(on_rx) = self.on_rx.as_mut() {
            (on_rx)(&p);
        }

        // Respond with OK
        Ok(NetResponseKind::Status(Status::Ok).into())
    }
}


#[cfg(test)]
mod test {
    use std::convert::Infallible;

    use dsf_core::prelude::*;
    use dsf_core::net::Status;
    use dsf_core::options::Metadata;
    
    use crate::endpoint::{self as ep};
    use crate::service::{IotService, IotData};

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
        let descriptors: Vec<Descriptor> = vec![
            (ep::Kind::Temperature, ep::Flags::R).into(),
            (ep::Kind::Pressure, ep::Flags::R).into(),
            (ep::Kind::Humidity, ep::Flags::R).into(),
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
            (NetRequestKind::Hello,                     NetResponseKind::Status(Status::Ok)),
            (NetRequestKind::Ping,                      NetResponseKind::Status(Status::Ok)),
            //(NetRequestKind::Query(e.svc.id()),         NetResponseKind::Status(Status::Ok)),
            //(NetRequestKind::Subscribe(e.svc.id()),     NetResponseKind::Status(Status::Ok)),
            //(NetRequestKind::Unsubscribe(e.svc.id()),   NetResponseKind::Status(Status::Ok)),
        ];

        for t in &tests {
            // Generate full request object
            let req = NetRequest::new(p.id(), 1, t.0.clone(), Flags::empty());

            // Pass to engine
            let resp = e.handle_req(&from, req.clone())
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
        let req = NetRequest::new(p.id(), 1, NetRequestKind::Subscribe(e.svc.id()), Flags::empty());
        let resp = e.handle_req(&from, req).expect("Failed to handle message");

        // Check response
        assert_eq!(resp, NetResponseKind::Status(Status::Ok).into());

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
        let req = NetRequest::new(p.id(), 1, NetRequestKind::Unsubscribe(e.svc.id()), Flags::empty());
        let resp = e.handle_req(&from, req).expect("Failed to handle message");

        // Check response
        let ex = NetResponse::new(e.svc.id(), 1, NetResponseKind::Status(Status::Ok), Flags::empty());
        assert_eq!(resp, NetResponseKind::Status(Status::Ok).into());

        // Check subscriber state
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscriber ), Some(false));
    }

    #[test]
    fn test_handle_discover() {
        let (p, mut e) = setup();
        let from = 1;

        // Build net request and execute
        let ep_filter: &[Descriptor] = &[(ep::Kind::Temperature, ep::Flags::R).into()];
        let (body, n) = ep_filter.encode_buff::<128>().unwrap();
        let req = NetRequest::new(p.id(), 1, NetRequestKind::Discover((&body[..n]).to_vec(), vec![]), Flags::empty());
        let resp = e.handle_req(&from, req).expect("Failed to handle message");

        // Check response
        let ex = NetResponse::new(e.svc.id(), 1, NetResponseKind::Status(Status::Ok), Flags::empty());
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
        let endpoint_data: [ep::Data<&'_ [Metadata]>; 3] = [
            ep::Data::new(27.3.into(), &[]),
            ep::Data::new(1016.2.into(), &[]),
            ep::Data::new(59.6.into(), &[]),
        ];

        let mut data_buff = [0u8; 128];
        let n = IotData::encode_data(&endpoint_data, &mut data_buff).unwrap();

        // Call publish operation
        e.publish(&data_buff[..n], &[])
            .expect("Publishing error");

        // Check outgoing data
        let d = e.comms.tx.pop().unwrap();
        assert_eq!(d.0, from);

        // Parse out page
        let (b, _n) = Base::parse(&d.1, &e.svc.keys()).expect("Failed to parse object");

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
        let (b, _n) = Base::parse(&d.1, &e.svc.keys()).expect("Failed to parse object");
        let m = NetMessage::convert(b, &e.store).expect("Failed to convert message");

        let expected = NetRequest::new(e.svc.id(), e.req_id, 
                NetRequestKind::Subscribe(p.id()), Flags::empty());

        assert_eq!( m, NetMessage::Request(expected), "Request mismatch");


        // Respond with subscribe ok
        let mut buff = [0u8; 512];
        let (_n, sp) = p.publish_primary(&mut buff).unwrap();

        let resp = NetResponse::new(p.id(), e.req_id, NetResponseKind::Status(Status::Ok), Flags::empty());
        e.handle_resp(&from, resp).expect("Response handling failed");

        // Check peer state is now subscribed
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscribed ), Some(SubscribeState::Subscribed));


        // Test receiving data
        let mut new_page = None;
        let mut h = |page: &Page| new_page = Some(page.clone()) ;
        e.set_handler(&mut h);

        e.handle_page(&from, sp.clone()).expect("Failed to handle page");
        assert_eq!(Some(sp), new_page);
    }



}