
use core::fmt::Debug;

use dsf_core::service::ServiceBuilder;

use super::{Engine, Store, EngineError};


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
impl Comms for MockComms {
    type Address = u8;

    type Error = core::convert::Infallible;

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
