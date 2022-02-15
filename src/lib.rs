#![cfg_attr(not(feature = "std"), no_std)]
#![feature(generic_associated_types)]
#![feature(const_generics_defaults)]
#![feature(trait_alias)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

pub mod endpoint;
pub mod error;
pub mod service;

pub mod prelude;

#[cfg(feature = "client")]
pub mod client;

pub mod engine;

pub const IOT_APP_ID: u16 = 1;

use dsf_core::api::Application;

pub struct IoT;


impl Application for IoT {
    const APPLICATION_ID: u16 = IOT_APP_ID;

    type Info = service::IotInfo;
    type Data = service::IotData;
}
