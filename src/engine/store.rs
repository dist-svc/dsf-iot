
use core::fmt::Debug;

use dsf_core::prelude::*;
use dsf_core::keys::{Keys, KeySource};
use dsf_core::types::ImmutableData;
use dsf_core::wire::Container;

bitflags::bitflags! {
    pub struct StoreFlags: u16 {
        const KEYS  = 0b0000_0001;
        const SIGS  = 0b0000_0010;
        const PAGES = 0b0000_0100;

        const ALL = Self::PAGES.bits() | Self::SIGS.bits() | Self::KEYS.bits();
    }
}

pub trait Store: KeySource {
    const FEATURES: StoreFlags;

    /// Peer address type
    type Address: Clone + Debug + 'static;

    /// Storage error type
    type Error: Debug;

    /// Peer iterator type, for collecting subscribers etc.
    type Iter<'a>: Iterator<Item=(&'a Id, &'a Peer<Self::Address>)>;


    /// Fetch keys associated with this service
    fn get_ident(&self) -> Option<Keys>;

    /// Set keys associated with this service
    fn set_ident(&mut self, keys: &Keys) -> Result<(), Self::Error>;


    /// Fetch last signature
    fn get_last_sig(&self) -> Option<Signature>;

    /// Update last signature
    fn set_last_sig(&mut self, sig: &Signature) -> Result<(), Self::Error>;

    
    // Fetch peer information
    fn get_peer(&self, id: &Id) -> Result<Option<Peer<Self::Address>>, Self::Error>;

    // Update a specified peer
    fn update_peer<R: Debug, F: Fn(&mut Peer<Self::Address>)-> R>(&mut self, id: &Id, f: F) -> Result<R, Self::Error>;

    // Iterate through known peers
    fn peers<'a>(&'a self) -> Self::Iter<'a>;


    // Store a page
    fn store_page<T: ImmutableData>(&mut self, sig: &Signature, p: &Container<T>) -> Result<(), Self::Error>;

    // Fetch a stored page
    fn fetch_page(&mut self, sig: &Signature) -> Result<Option<Container>, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct Peer<Addr: Clone + Debug> {
    pub keys: Keys,                     // Key storage for the peer / service
    pub addr: Option<Addr>,             // Optional address for the peer / service
    pub subscriber: bool,               // Indicate whether this service is subscribed to us
    pub subscribed: SubscribeState,     // Indicate whether we are subscribed to this service
}

impl <Addr: Clone + Debug> Default for Peer<Addr> {
    fn default() -> Self {
        Self { 
            keys: Keys::default(), 
            addr: None,
            subscriber: false,
            subscribed: SubscribeState::None,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SubscribeState {
    None,
    Subscribing(RequestId),
    Subscribed,
    Unsubscribing(RequestId),
}

impl <Addr: Clone + Debug> Peer<Addr> {
    pub fn subscribed(&self) -> bool {
        use SubscribeState::*;

        if let Subscribing(_) = self.subscribed {
            true
        } else if self.subscribed == Subscribed {
            true
        } else {
            false
        }
    }
}

pub struct MemoryStore<Addr: Clone + Debug = std::net::SocketAddr> {
    pub(crate) our_keys: Option<Keys>,
    pub(crate) last_sig: Option<Signature>,
    pub(crate) peers: std::collections::HashMap<Id, Peer<Addr>>,
    pub(crate) pages: std::collections::HashMap<Signature, Container>
}

impl <Addr: Clone + Debug> MemoryStore<Addr> {
    pub fn new() -> Self {
        Self {
            our_keys: None,
            last_sig: None,
            peers: std::collections::HashMap::new(),
            pages: std::collections::HashMap::new(),
        }
    }
}

impl <Addr: Clone + Debug + 'static> Store for MemoryStore<Addr> {
    const FEATURES: StoreFlags = StoreFlags::ALL;

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

    fn get_last_sig(&self) -> Option<Signature> {
        self.last_sig.clone()
    }

    fn set_last_sig(&mut self, sig: &Signature) -> Result<(), Self::Error> {
        self.last_sig = Some(sig.clone());
        Ok(())
    }

    fn get_peer(&self, id: &Id) -> Result<Option<Peer<Self::Address>>, Self::Error> {
        let p = self.peers.get(id);
        Ok(p.map(|p| p.clone() ))
    }

    fn peers<'a>(&'a self) -> Self::Iter<'a> {
        self.peers.iter()
    }

    fn update_peer<R: Debug, F: Fn(&mut Peer<Self::Address>)-> R>(&mut self, id: &Id, f: F) -> Result<R, Self::Error> {
        let p = self.peers.entry(id.clone()).or_default();
        Ok(f(p))
    }

    fn store_page<T: ImmutableData>(&mut self, sig: &Signature, p: &Container<T>) -> Result<(), Self::Error> {
        self.pages.insert(sig.clone(), p.to_owned());
        Ok(())
    }

    fn fetch_page(&mut self, sig: &Signature) -> Result<Option<Container>, Self::Error> {
        let p = self.pages.get(sig).map(|p| p.clone() );
        Ok(p)
    }
}

impl <'a, Addr: Clone + Debug + 'static> IntoIterator for &'a MemoryStore<Addr>{
    type Item = (&'a Id, &'a Peer<Addr>);

    type IntoIter = std::collections::hash_map::Iter<'a, Id, Peer<Addr>>;

    fn into_iter(self) -> Self::IntoIter {
        self.peers.iter()
    }
}


impl <Addr: Clone + Debug> KeySource for MemoryStore<Addr> {
    fn keys(&self, id: &Id) -> Option<Keys> {
        self.peers.get(id).map(|p| p.keys.clone() )
    }
}
