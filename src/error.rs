#[derive(Debug)]
pub enum IotError {
    Core(dsf_core::error::Error),

    #[cfg(feature = "std")]
    Client(dsf_client::Error),

    NoSecretKey,
    NoBody,
    UnrecognisedEndpoint,
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
