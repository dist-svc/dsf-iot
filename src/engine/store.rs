
use core::fmt::Debug;
use std::marker::PhantomData;

use byteorder::{LittleEndian, ByteOrder};
use dsf_core::prelude::*;
use dsf_core::keys::{Keys, KeySource};
use dsf_core::types::{ImmutableData, SIGNATURE_LEN};
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
    fn get_ident(&self) -> Result<Option<Keys>, Self::Error>;

    /// Set keys associated with this service
    fn set_ident(&mut self, keys: &Keys) -> Result<(), Self::Error>;


    /// Fetch previous object information
    fn get_last(&self) -> Result<Option<ObjectInfo>, Self::Error>;

    /// Update previous object information
    fn set_last(&mut self, info: &ObjectInfo) -> Result<(), Self::Error>;

    
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
pub struct ObjectInfo {
    pub page_index: u16,
    pub block_index: u16,
    pub sig: Signature,
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
    pub(crate) last_sig: Option<ObjectInfo>,
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

    fn get_ident(&self) -> Result<Option<Keys>, Self::Error> {
        Ok(self.our_keys.clone())
    }

    fn set_ident(&mut self, keys: &Keys) -> Result<(), Self::Error> {
        self.our_keys = Some(keys.clone());
        Ok(())
    }

    /// Fetch previous object information
    fn get_last(&self) -> Result<Option<ObjectInfo>, Self::Error> {
        Ok(self.last_sig.clone())
    }

    /// Update previous object information
    fn set_last(&mut self, info: &ObjectInfo) -> Result<(), Self::Error> {
        self.last_sig = Some(info.clone());
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

pub struct SledStore<Addr: Clone + Debug> {
    db: sled::Db,
    peers: std::collections::HashMap<Id, Peer<Addr>>,
    _addr: PhantomData<Addr>,
}

impl <Addr: Clone + Debug> SledStore<Addr> {
    /// Create a new sled-backed store
    pub fn new(path: &str) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        let peers = std::collections::HashMap::new();

        Ok(Self{db, peers, _addr: PhantomData})
    }
}

const SLED_IDENT_KEY: &[u8] = b"ident";
const SLED_PAGE_KEY: &[u8] = b"page";
const SLED_LAST_KEY: &[u8] = b"last";

impl <Addr: Clone + Debug + 'static> Store for SledStore<Addr> {
    const FEATURES: StoreFlags = StoreFlags::ALL;

    type Address = Addr;

    type Error = sled::Error;

    type Iter<'a> = std::collections::hash_map::Iter<'a, Id, Peer<Addr>>;

    fn get_ident(&self) -> Result<Option<Keys>, Self::Error> {
        let ident = self.db.open_tree(SLED_IDENT_KEY)?;

        let mut keys = Keys::default();

        if let Some(pri_key) = ident.get("pri_key")? {
            let pri_key = PrivateKey::from(pri_key.as_ref());

            keys.pub_key = Some(dsf_core::crypto::pk_derive(&pri_key).unwrap());
            keys.pri_key = Some(pri_key);
        }

        if let Some(sec_key) = ident.get("sec_key")? {
            keys.sec_key = Some(SecretKey::from(sec_key.as_ref()));
        }

        match keys.pub_key.is_some() {
            true => Ok(Some(keys)),
            false => Ok(None),
        }
    }

    fn set_ident(&mut self, keys: &Keys) -> Result<(), Self::Error> {
        let ident = self.db.open_tree(SLED_IDENT_KEY)?;

        if let Some(pri_key) = keys.pri_key.as_deref() {
            ident.insert("pri_key", pri_key)?;
        }

        if let Some(sec_key) = keys.sec_key.as_deref() {
            ident.insert("sec_key", sec_key)?;
        }

        Ok(())
    }

    fn get_last(&self) -> Result<Option<ObjectInfo>, Self::Error> {
        match self.db.get(SLED_LAST_KEY)? {
            Some(k) => {
                let d = k.as_ref();

                Ok(Some(ObjectInfo{
                    page_index: LittleEndian::read_u16(&k[0..]),
                    block_index: LittleEndian::read_u16(&k[2..]),
                    sig: Signature::from(&k[4..]),
                }))
            },
            None => Ok(None),
        }
    }

    fn set_last(&mut self, info: &ObjectInfo) -> Result<(), Self::Error> {
        let mut d = [0u8; 2 + 2 + SIGNATURE_LEN];

        LittleEndian::write_u16(&mut d[0..], info.page_index);
        LittleEndian::write_u16(&mut d[2..], info.block_index);
        d[4..].copy_from_slice(&info.sig);

        self.db.insert(SLED_LAST_KEY, d.as_slice())?;

        Ok(())
    }

    fn get_peer(&self, id: &Id) -> Result<Option<Peer<Addr>>, Self::Error> {
        let p = self.peers.get(id);
        Ok(p.map(|p| p.clone() ))
    }

    fn peers<'a>(&'a self) -> Self::Iter<'a> {
        self.peers.iter()
    }

    fn update_peer<R: Debug, F: Fn(&mut Peer<Addr>)-> R>(&mut self, id: &Id, f: F) -> Result<R, Self::Error> {
        let p = self.peers.entry(id.clone()).or_default();
        Ok(f(p))
    }

    fn store_page<T: ImmutableData>(&mut self, sig: &Signature, p: &Container<T>) -> Result<(), Self::Error> {
        let pages = self.db.open_tree(SLED_PAGE_KEY)?;

        pages.insert(sig, p.raw())?;

        Ok(())
    }

    fn fetch_page(&mut self, sig: &Signature) -> Result<Option<Container>, Self::Error> {
        let pages = self.db.open_tree(SLED_PAGE_KEY)?;

        match pages.get(sig)? {
            Some(p) => {
                let (c, _n) = Container::from(p.as_ref().to_vec());
                Ok(Some(c))
            },
            None => Ok(None),
        }
    }
}

impl <Addr: Clone + Debug> KeySource for SledStore<Addr> {
    fn keys(&self, id: &Id) -> Option<Keys> {
        self.peers.get(id).map(|p| p.keys.clone() )
    }
}
