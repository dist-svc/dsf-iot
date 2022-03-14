#[derive(Debug)]
#[cfg_attr(feature="thiserror", derive(thiserror::Error))]
#[cfg_attr(feature="defmt", derive(defmt::Format))]
pub enum IotError {
    #[cfg_attr(feature="thiserror", error("core error: {0}"))]
    Core(dsf_core::error::Error),

    #[cfg(feature = "std")]
    #[cfg_attr(feature="thiserror", error("client error: {0}"))]
    Client(dsf_client::Error),

    #[cfg(feature = "std")]
    #[cfg_attr(feature="thiserror", error("io error: {0}"))]
    Io(std::io::Error),

    #[cfg_attr(feature="thiserror", error("No secret key for service"))]
    NoSecretKey,

    #[cfg_attr(feature="thiserror", error("IoT object missing body"))]
    NoBody,

    #[cfg_attr(feature="thiserror", error("Unrecognised endpoint kind"))]
    UnrecognisedEndpoint,

    #[cfg_attr(feature="thiserror", error("Overrun in static vector"))]
    Overrun,
}

#[cfg(feature = "std")]
impl From<dsf_client::Error> for IotError {
    fn from(e: dsf_client::Error) -> Self {
        Self::Client(e)
    }
}

impl From<dsf_core::error::Error> for IotError {
    fn from(o: dsf_core::error::Error) -> Self {
        Self::Core(o)
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for IotError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}