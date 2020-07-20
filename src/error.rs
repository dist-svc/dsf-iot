

#[derive(Debug)]
pub enum IotError {
    #[cfg(feature = "std")]
    Client(dsf_client::Error),

    Options(dsf_core::options::OptionsError),

    NoSecretKey,
    NoBody,
}


#[cfg(feature = "std")]
impl From<dsf_client::Error> for IotError {
    fn from(e: dsf_client::Error) -> Self {
        Self::Client(e)
    }
}

impl From<dsf_core::options::OptionsError> for IotError {
    fn from(o: dsf_core::options::OptionsError) -> Self {
        Self::Options(o)
    }
}
