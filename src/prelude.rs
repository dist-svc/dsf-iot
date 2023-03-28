pub use crate::endpoint::{Descriptor as EpDescriptor, Data as EpData, Kind as EpKind, Flags as EpFlags, IotData, IotInfo};

pub use crate::service::{IotService};

#[cfg(feature = "client")]
pub use crate::client::{options::*, IotClient, Options, ServiceIdentifier};

pub use crate::error::IotError;

pub use crate::{IoT, IotEngine};