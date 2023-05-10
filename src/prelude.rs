//! Prelude to simplify use of `dsf_iot` crate

pub use crate::endpoint::{EpData, EpDescriptor, EpFlags, EpKind, IotData, IotInfo};

#[cfg(feature = "client")]
pub use crate::client::{options::*, Config, IotClient, ServiceIdentifier};

pub use crate::error::IotError;

pub use crate::{IoT, IotEngine};
