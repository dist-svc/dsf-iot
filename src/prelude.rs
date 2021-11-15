pub use crate::endpoint::{Descriptor, Data, Kind, Flags as Flags};

pub use crate::service::{IotData, IotService};

#[cfg(feature = "client")]
pub use crate::client::{options::*, IotClient, Options, ServiceIdentifier};

pub use crate::error::IotError;
