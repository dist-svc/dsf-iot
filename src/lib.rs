
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc", feature(alloc_prelude))]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

pub mod endpoint;
pub mod service;
pub mod error;

pub mod prelude;

#[cfg(feature = "client")]
pub mod client;

pub const IOT_APP_ID: u16 = 1;

pub const IOT_SERVICE_PAGE_KIND: u16 = 1;
pub const IOT_DATA_PAGE_KIND: u16 = 2;
