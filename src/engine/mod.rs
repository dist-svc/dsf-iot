use core::fmt::Debug;
use core::convert::TryFrom;

use std::collections::HashMap;

use log::{debug, warn, error};

use dsf_core::{prelude::*, options::Options, net::Status};

use crate::{IOT_APP_ID, endpoint::Descriptor};

// Trying to build an abstraction over IP, LPWAN, (UNIX to daemon?)

pub struct Engine<C: Comms, S: Store = MemoryStore> {
    svc: Service,
    comms: C,
    store: S,
}

pub trait Comms {
    /// Address for directing packets
    type Address: Debug;

    // Communication error type
    type Error: Debug;

    /// Receive data if available
    fn recv(&mut self, buff: &mut [u8]) -> Result<Option<(usize, Self::Address)>, Self::Error>;

    /// Send data to the specified address
    fn send(&mut self, to: &Self::Address, data: &[u8]) -> Result<(), Self::Error>;

    /// Broadcast data
    fn broadcast(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}

pub trait Store: KeySource {
    /// Peer address type
    type Address: Debug + 'static;

    /// Storage error type
    type Error: Debug;

    type Iter<'a>: Iterator<Item=(&'a Id, &'a Peer<Self::Address>)>;

    /// Fetch keys associated with this service
    fn get_ident(&self) -> Option<Keys> {
        None
    }

    /// Set keys associated with this service
    fn set_ident(&mut self, _keys: &Keys) -> Result<(), Self::Error> {
        Ok(())
    }

    fn get_last_sig(&self) -> Option<Signature> {
        None
    }

    fn set_last_sig(&mut self, sig: &Signature) -> Result<(), Self::Error> {
        todo!()
    }

    fn update_peer<F: Fn(&mut Peer<Self::Address>)-> ()>(&mut self, id: &Id, f: F) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn peers<'a>(&'a self) -> Self::Iter<'a>{
        todo!()
    }
}

#[derive(Debug)]
pub struct Peer<Addr: Debug> {
    pub keys: Keys,
    pub addr: Option<Addr>,
    pub subscriber: bool,
    pub subscribed: bool,
}

impl <Addr: Debug> Default for Peer<Addr> {
    fn default() -> Self {
        Self { 
            keys: Keys::default(), 
            addr: None,
            subscriber: false,
            subscribed: false,
        }
    }
}

pub struct MemoryStore<Addr: Debug = std::net::SocketAddr> {
    our_keys: Option<Keys>,
    peers: HashMap<Id, Peer<Addr>>,
}

impl <Addr: Debug> MemoryStore<Addr> {
    pub fn new() -> Self {
        Self {
            our_keys: None,
            peers: HashMap::new(),
        }
    }
}

impl <Addr: Debug + 'static> Store for MemoryStore<Addr> {
    type Address = Addr;
    type Error = core::convert::Infallible;
    type Iter<'a> = std::collections::hash_map::Iter<'a, Id, Peer<Addr>>;

    fn get_ident(&self) -> Option<Keys> {
        self.our_keys.clone()
    }

    fn set_ident(&mut self, keys: &Keys) -> Result<(), Self::Error> {
        self.our_keys = Some(keys.clone());
        Ok(())
    }

    fn update_peer<F: Fn(&mut Peer<Self::Address>)-> ()>(&mut self, id: &Id, f: F) -> Result<(), Self::Error> {
        let p = self.peers.entry(id.clone()).or_default();
        f(p);
        Ok(())
    }

    fn peers<'a>(&'a self) -> Self::Iter<'a> {
        self.peers.iter()
    }
}

impl <'a, Addr: 'static + Debug> IntoIterator for &'a MemoryStore<Addr>{
    type Item = (&'a Id, &'a Peer<Addr>);

    type IntoIter = std::collections::hash_map::Iter<'a, Id, Peer<Addr>>;

    fn into_iter(self) -> Self::IntoIter {
        self.peers.iter()
    }
}


impl <Addr: Debug> KeySource for MemoryStore<Addr> {
    fn keys(&self, id: &Id) -> Option<Keys> {
        self.peers.get(id).map(|p| p.keys.clone() )
    }
}

#[derive(Debug)]
pub enum EngineError<CommsError: Debug, StoreError: Debug> {
    Core(dsf_core::error::Error),
    
    Comms(CommsError),

    Store(StoreError),

    Unhandled,
}

pub enum EngineEvent {

}



impl <A: Clone + Debug, C: Comms<Address=A>, S: Store<Address=A>> Engine<C, S> {

    pub fn new(mut sb: ServiceBuilder, comms: C, store: S) -> Result<Self, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        // Start assembling the service
        sb = sb.application_id(IOT_APP_ID);

        // Attempt to load existing keys
        if let Some(k) = store.get_ident() {
            sb = sb.keys(k);
        }

        // Attempt to load last sig for continuation
        // TODO: should this fetch the index too?
        if let Some(s) = store.get_last_sig() {
            sb = sb.last_signature(s);
        }

        // Create service
        let svc = sb.build().map_err(EngineError::Core)?;

        // Return object
        Ok(Self{ svc, comms, store })
    }

    /// Publish service data
    pub fn publish(&mut self, body: &[u8], opts: &[Options]) -> Result<(), EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        
        // Setup page options for encoding
        let page_opts = DataOptions {
            body: Body::Cleartext(body.to_vec()),
            public_options: opts.to_vec(),
            ..Default::default()
        };

        // Publish data to buffer
        let mut page_buff = [0u8; 512];
        let (n, p) = self.svc.publish_data(page_opts, &mut page_buff[..]).map_err(EngineError::Core)?;

        let data = &page_buff[..n];

        // TODO: write to store

        // Send updated page to subscribers
        for (id, p) in self.store.peers() {
            match (&p.subscriber, &p.addr) {
                (true, Some(addr)) => {
                    debug!("Forwarding data to: {} ({:?})", id, addr);
                    self.comms.send(addr, data).map_err(EngineError::Comms)?;
                },
                _ => (),
            }
        }

        Ok(())
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
        match (NetMessage::convert(base.clone(), &self.store), Page::try_from(base)) {
            (Ok(NetMessage::Request(req)), _) => self.handle_req(from, req)?,
            (Ok(NetMessage::Response(resp)), _) => self.handle_resp(from, resp)?,
            (_, Ok(p)) => self.handle_page(from, p)?,
            _ => {
                error!("Unhandled object type");
                return Err(EngineError::Unhandled)
            }
        };

        // TODO: send responses

        todo!()
    }

    fn handle_req(&mut self, from: A, req: NetRequest) -> Result<Option<NetResponse>, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        use NetRequestKind::*;

        debug!("Received request: {:?} from: {:?}", req, from);

        // Handle request messages
        let resp_body = match &req.data {
            Hello | Ping => Some(NetResponseKind::Status(Status::Ok)),
            //Discover => (),
            Subscribe(id) if id == &self.svc.id() => {
                debug!("Adding {} ({:?}) as a subscriber", req.common.from, from);

                self.store.update_peer(&req.common.from, |p| {
                    p.subscriber = true;
                    p.addr = Some(from.clone());
                }).map_err(EngineError::Store)?;

                Some(NetResponseKind::Status(Status::Ok))
            },
            Unsubscribe(id) if id == &self.svc.id() => {
                debug!("Removing {} ({:?}) as a subscriber", req.common.from, from);

                self.store.update_peer(&req.common.from, |p| {
                    p.subscriber = false;
                }).map_err(EngineError::Store)?;

                Some(NetResponseKind::Status(Status::Ok))
            },
            Subscribe(_id) | Unsubscribe(_id) => {
                Some(NetResponseKind::Status(Status::InvalidRequest))
            },
            //PushData(_, _) => (),
            _ => Some(NetResponseKind::Status(Status::InvalidRequest)),
        };

        if let Some(b) = resp_body {
            Ok(Some(NetResponse::new(self.svc.id(), req.id, b, Flags::empty())))
        } else {
            Ok(None)
        }
    }

    fn handle_resp(&mut self, from: <C as Comms>::Address, resp: NetResponse) -> Result<Option<NetResponse>, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        use NetResponseKind::*;

        debug!("Received response: {:?} from: {:?}", resp, from);

        // Handle response messages
        match &resp.data {
            Status(_) => (),
            NoResult => (),
            PullData(_, _) => (),
            _ => todo!(),
        };


        todo!()
    }

    fn handle_page(&mut self, from: <C as Comms>::Address, p: Page) -> Result<Option<NetResponse>, EngineError<<C as Comms>::Error, <S as Store>::Error>> {
        debug!("Received page: {:?} from: {:?}", p, from);

        todo!()
    }
}

#[cfg(feature="std")]
impl <S: Store<Address=std::net::SocketAddr>> Engine<std::net::UdpSocket, S> {
    /// Create a new UDP engine instance
    pub fn udp<A: std::net::ToSocketAddrs>(sb: ServiceBuilder, addr: A, store: S) -> Result<Self, EngineError<std::io::Error, <S as Store>::Error>> {
        // Attempt to bind UDP socket
        let comms = std::net::UdpSocket::bind(addr).map_err(EngineError::Comms)?;

        // Enable broadcast and nonblocking polling
        comms.set_broadcast(true).map_err(EngineError::Comms)?;
        comms.set_nonblocking(true).map_err(EngineError::Comms)?;

        // Create engine instance
        Self::new(sb, comms, store)
    }

    // Tick function to update engine
    pub fn tick(&mut self) -> Result<(), EngineError<std::io::Error, <S as Store>::Error>> {
        let mut buff = [0u8; 512];

        // Check for and handle received messages
        if let Some((n, a)) = Comms::recv(&mut self.comms, &mut buff).map_err(EngineError::Comms)? {
            self.handle(a, &buff[..n])?;
        }

        // TODO: anything else?

        Ok(())
    }
}

#[cfg(feature="std")]
impl Comms for std::net::UdpSocket {
    type Address = std::net::SocketAddr;

    type Error = std::io::Error;

    fn recv(&mut self, buff: &mut [u8]) -> Result<Option<(usize, Self::Address)>, Self::Error> {
        match self.recv_from(buff) {
            Ok(v) => Ok(Some(v)),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn send(&mut self, to: &Self::Address, data: &[u8]) -> Result<(), Self::Error> {
        self.send_to(data, to)?;
        Ok(())
    }

    fn broadcast(&mut self, _data: &[u8]) -> Result<(), Self::Error> {
        todo!("Work out how to derive broadcast address")
    }
}


#[cfg(test)]
mod test {
    use std::convert::Infallible;

    use dsf_core::prelude::*;
    use dsf_core::net::Status;
    
    use crate::endpoint::{self as ep};
    use crate::service::{IotService, IotData};

    use super::*;

    struct MockComms {
        tx: Vec<(u8, Vec<u8>)>,
    }

    impl Default for MockComms {
        fn default() -> Self {
            Self { tx: vec![] }
        }
    }

    impl Comms for MockComms {
        type Address = u8;

        type Error = Infallible;

        fn recv(&mut self, buff: &mut [u8]) -> Result<Option<(usize, Self::Address)>, Self::Error> {
            todo!()
        }

        fn send(&mut self, to: &Self::Address, data: &[u8]) -> Result<(), Self::Error> {
            self.tx.push((*to, data.to_vec()));
            Ok(())
        }

        fn broadcast(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            todo!()
        }
    }

    // Setup an engine instance for testing
    fn setup() -> (Service, Engine<MockComms, MemoryStore<u8>>) {
        // Setup debug logging
        let _ = simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, simplelog::Config::default());

        // Create peer for sending requests
        let p = ServiceBuilder::generic().build().unwrap();

        // Setup memory store with pre-filled peer keys
        let mut s = MemoryStore::<u8>::new();
        s.update(&p.id(), |k| *k = p.keys() );

        // Setup engine with default service
        let e = Engine::new(ServiceBuilder::generic(), MockComms::default(), s)
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
            let resp = e.handle_req(from, req.clone())
                .expect("Failed to handle message");

            // Check response
            let ex = NetResponse::new(e.svc.id(), 1, t.1.clone(), Flags::empty());
            assert_eq!(resp.as_ref(), Some(&ex),
                "\nRequest: {:#?}\nExpected: {:#?}\nActual: {:#?}", req, ex, resp);
        }
    }

    #[test]
    fn test_handle_subscribe() {
        let (p, mut e) = setup();
        let from = 1;

        // Build subscribe request and execute
        let req = NetRequest::new(p.id(), 1, NetRequestKind::Subscribe(e.svc.id()), Flags::empty());
        let resp = e.handle_req(from, req).expect("Failed to handle message");

        // Check response
        let ex = NetResponse::new(e.svc.id(), 1, NetResponseKind::Status(Status::Ok), Flags::empty());
        assert_eq!(resp, Some(ex));

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
        let resp = e.handle_req(from, req).expect("Failed to handle message");

        // Check response
        let ex = NetResponse::new(e.svc.id(), 1, NetResponseKind::Status(Status::Ok), Flags::empty());
        assert_eq!(resp, Some(ex));

        // Check subscriber state
        assert_eq!(e.store.peers.get(&p.id()).map(|p| p.subscriber ), Some(false));
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
        let endpoint_data = [
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

        let (b, _n) = Base::parse(&d.1, &e.svc.keys()).expect("Failed to parse object");

        // TODO: translate back to IoT data

    }
}