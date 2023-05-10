//! Prelude to simplify use of `dsf_iot` crate

pub use crate::endpoint::{
    Data as EpData, Descriptor as EpDescriptor, Flags as EpFlags, IotData, IotInfo, Kind as EpKind,
};

#[cfg(feature = "client")]
pub use crate::client::{options::*, Config, IotClient, ServiceIdentifier};

pub use crate::error::IotError;

pub use crate::{IoT, IotEngine};
