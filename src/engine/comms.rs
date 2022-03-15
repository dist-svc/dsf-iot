

use dsf_core::{base::PageBody, api::Application};

use crate::log::{Debug, trace, debug, info, warn, error};
use crate::prelude::EpDescriptor;

use super::{Engine, Store, EngineError, EngineEvent};


pub trait Communications {
    /// Address for directing packets
    type Address: Debug;

    /// Communication error type
    type Error: Debug;

    /// Receive data if available
    fn recv(&mut self, buff: &mut [u8]) -> Result<Option<(usize, Self::Address)>, Self::Error>;

    /// Send data to the specified address
    fn send(&mut self, to: &Self::Address, data: &[u8]) -> Result<(), Self::Error>;

    /// Broadcast data
    fn broadcast(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}



#[cfg(feature="std")]
impl <App: Application, S: Store<Address=std::net::SocketAddr>, const N: usize> Engine<App, std::net::UdpSocket, S, N> {
    /// Create a new UDP engine instance
    pub fn udp<A: std::net::ToSocketAddrs + Debug>(info: App::Info, addr: A, store: S) -> Result<Self, EngineError<std::io::Error, <S as Store>::Error>> {
        debug!("Connecting to socket: {:?}", addr);

        // Attempt to bind UDP socket
        let comms = std::net::UdpSocket::bind(addr).map_err(EngineError::Comms)?;

        // Enable broadcast and nonblocking polling
        comms.set_broadcast(true).map_err(EngineError::Comms)?;
        comms.set_nonblocking(true).map_err(EngineError::Comms)?;

        // Create engine instance
        Self::new(info, comms, store)
    }

    // Tick function to update engine
    pub fn tick(&mut self) -> Result<EngineEvent, EngineError<std::io::Error, <S as Store>::Error>> {
        let mut buff = [0u8; N];

        // Check for and handle received messages
        if let Some((n, a)) = Communications::recv(&mut self.comms, &mut buff).map_err(EngineError::Comms)? {
            debug!("Received {} bytes from {:?}", n, a);
            return self.handle(a, &mut buff[..n]);
        }

        // Update internal state
        return self.update();
    }

    pub fn addr(&mut self) -> Result<std::net::SocketAddr, EngineError<std::io::Error, <S as Store>::Error>>{
        let a = self.comms.local_addr().map_err(EngineError::Comms)?;
        Ok(a)
    }
}

#[cfg(feature="std")]
impl Communications for std::net::UdpSocket {
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

    fn broadcast(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        use std::net::{SocketAddr, Ipv4Addr};
        
        let a = match self.local_addr()? {
            SocketAddr::V4(mut v4) => {
                v4.set_ip(Ipv4Addr::new(255, 255, 255, 255));
                v4
            },
            _ => unimplemented!(),
        };

        debug!("Broadcast {} bytes to: {}", data.len(), a);

        self.send_to(data, a)?;

        Ok(())
    }
}

#[cfg(test)]
pub struct MockComms {
    pub(crate) tx: Vec<(u8, Vec<u8>)>,
}

#[cfg(test)]
impl Default for MockComms {
    fn default() -> Self {
        Self { tx: vec![] }
    }
}

#[cfg(test)]
impl Communications for MockComms {
    type Address = u8;

    type Error = core::convert::Infallible;

    fn recv(&mut self, _buff: &mut [u8]) -> Result<Option<(usize, Self::Address)>, Self::Error> {
        todo!()
    }

    fn send(&mut self, to: &Self::Address, data: &[u8]) -> Result<(), Self::Error> {
        self.tx.push((*to, data.to_vec()));
        Ok(())
    }

    fn broadcast(&mut self, _data: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }
}
