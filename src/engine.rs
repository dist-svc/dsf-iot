use core::fmt::Debug;
use core::convert::TryFrom;

use log::{debug, warn, error};

use dsf_core::{prelude::*, options::Options};

use crate::{IOT_APP_ID, prelude::*};

// Trying to build an abstraction over IP, LPWAN, (UNIX to daemon?)

pub struct Engine<C: Comms, S: Store = Keys, K: KeySource = Keys> {
    svc: Service,
    comms: C,
    store: S,
    keys: K,
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

pub trait Store {
    /// Storage error type
    type Error: Debug;

    /// Fetch keys associated with this service
    fn get_keys(&self) -> Option<Keys> {
        None
    }

    /// Set keys associated with this service
    fn set_keys(&mut self, _keys: &Keys) -> Result<(), Self::Error> {
        Ok(())
    }

    fn get_last_sig(&self) -> Option<Signature> {
        None
    }

    fn set_last_sig(&mut self, sig: &Signature) -> Result<(), Self::Error> {
        todo!()
    }
}

impl Store for Keys {
    type Error = core::convert::Infallible;

    fn get_keys(&self) -> Option<Keys> {
        Some(self.clone())
    }

    fn set_keys(&mut self, keys: &Keys) -> Result<(), Self::Error> {
        *self = keys.clone();
        Ok(())
    } 
}

#[derive(Debug)]
pub enum EngineError {
    Core(dsf_core::error::Error),
    #[cfg(feature="std")]
    Io(std::io::Error),
    Unhandled,
}

pub enum EngineEvent {

}



impl <C: Comms, S: Store, K: KeySource> Engine<C, S, K> {

    pub fn new(mut sb: ServiceBuilder, comms: C, store: S, keys: K) -> Result<Self, EngineError> {
        // Start assembling the service
        sb = sb.application_id(IOT_APP_ID);

        // Attempt to load existing keys
        if let Some(k) = store.get_keys() {
            sb = sb.keys(k);
        }

        // TODO: attempt to load last sig / index for continuation

        // Create service
        let svc = sb.build().map_err(EngineError::Core)?;

        Ok(Self{ svc, comms, store, keys })
    }

    /// Handle received data
    pub fn handle(&mut self, from: <C as Comms>::Address, data: &[u8]) -> Result<(), EngineError> {
        debug!("Received {} bytes from {:?}", data.len(), from);

        // Parse base object
        let (base, _n) = match Base::parse(data, &self.keys) {
            Ok(v) => (v),
            Err(e) => {
                error!("DSF parsing error: {:?}", e);
                return Err(EngineError::Core(e))
            }
        };

        // Convert and handle messages
        match (NetMessage::convert(base.clone(), &self.keys), Page::try_from(base)) {
            (Ok(m), _) => self.handle_message(from, m)?,
            (_, Ok(p)) => self.handle_page(from, p)?,
            _ => {
                error!("Unhandled object type");
                return Err(EngineError::Unhandled)
            }
        };

        todo!()
    }

    fn handle_message(&mut self, from: <C as Comms>::Address, m: NetMessage) -> Result<(), EngineError> {

        debug!("Received message: {:?} from: {:?}", m, from);

        match &m {
            NetMessage::Request(req) => {
                use NetRequestKind::*;

                // Handle request messages
                let resp = match &req.data {
                    Hello => (),
                    Ping => (),
                    Discover => (),
                    Subscribe(_) => (),
                    PushData(_, _) => (),
                    _ => todo!(),
                };

                // TODO: send response
            },
            NetMessage::Response(resp) => {
                use NetResponseKind::*;

                // Handle response messages
                match &resp.data {
                    Status(_) => (),
                    NoResult => (),
                    PullData(_, _) => (),
                    _ => todo!(),
                };

            },   
        }

        todo!()
    }

    fn handle_page(&mut self, from: <C as Comms>::Address, p: Page) -> Result<(), EngineError> {
        debug!("Received page: {:?} from: {:?}", p, from);

        todo!()
    }
}

#[cfg(feature="std")]
impl <S: Store, K: KeySource> Engine<std::net::UdpSocket, S, K> {
    pub fn udp<A: std::net::ToSocketAddrs>(sb: ServiceBuilder, addr: A, store: S, keys: K) -> Result<Self, EngineError> {
        // Attempt to bind UDP socket
        let comms = std::net::UdpSocket::bind(addr).map_err(EngineError::Io)?;

        // Create engine instance
        Self::new(sb, comms, store, keys)
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

}