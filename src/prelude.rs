pub use crate::endpoint::{Descriptor as EpDescriptor, Data as EpData, Kind as EpKind, Flags as EpFlags};

pub use crate::service::{IotData, IotService};

#[cfg(feature = "client")]
pub use crate::client::{options::*, IotClient, Options, ServiceIdentifier};

pub use crate::error::IotError;
