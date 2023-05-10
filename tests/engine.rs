use std::net::{SocketAddr, UdpSocket};

use encdec::EncodeExt;
use log::info;

use dsf_core::prelude::*;
use dsf_iot::prelude::*;

use dsf_engine::{engine::EngineEvent, store::MemoryStore};

type E = IotEngine<UdpSocket, MemoryStore, 512>;

use ctor::ctor;

#[ctor]
fn init_color_backtrace() {
    color_backtrace::install();
}

fn new_engine(addr: &str, descriptors: Vec<EpDescriptor>) -> anyhow::Result<E> {
    // Create peer for sending requests
    let p = ServiceBuilder::<IotInfo>::generic().build()?;

    // Setup memory store with pre-filled peer keys
    let mut s = MemoryStore::<SocketAddr>::new();
    s.update(&p.id(), |k| *k = p.keys());

    // Setup engine with newly created service
    let e = E::udp(IotInfo::new(&descriptors).unwrap(), &[], addr, s)?;

    Ok(e)
}

#[test]
fn integration() -> anyhow::Result<()> {
    // Setup debug logging
    let log_cfg = simplelog::ConfigBuilder::new()
        //.add_filter_ignore_str("dsf_core::wire")
        .build();
    let _ = simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, log_cfg);

    // Setup descriptors
    let descriptors = vec![
        EpDescriptor::new(EpKind::Temperature, EpFlags::R),
        EpDescriptor::new(EpKind::Pressure, EpFlags::R),
        EpDescriptor::new(EpKind::Humidity, EpFlags::R),
    ];

    // Create a pair of engines
    let mut e1 = new_engine("127.0.0.1:11000", descriptors.clone())?;
    let mut e2 = new_engine("127.0.0.2:11000", descriptors.clone())?;

    // Tick engines to get started
    e1.tick()?;
    e2.tick()?;

    info!("Attempting discovery");

    // Attempt local service discovery
    let ep_filter: &[EpDescriptor] = &[EpDescriptor::new(EpKind::Temperature, EpFlags::R)];
    let (body, n) = ep_filter.encode_buff::<128>().unwrap();
    e1.discover(&body[..n], &[])?;

    // Tick to update discovery state
    e1.tick()?;
    e2.tick()?;

    e1.tick()?;
    e2.tick()?;

    // TODO: broadcast doesn't seem to be working here..? 127.0.0.x address maybe?
    // hack to fix for now

    info!("Starting subscribe");

    // Attempt subscription
    e1.subscribe(e2.id(), e2.addr()?)?;

    // Tick to update discovery state
    assert_eq!(e2.tick()?, EngineEvent::SubscribeFrom(e1.id()));
    assert_eq!(e1.tick()?, EngineEvent::SubscribedTo(e2.id()));

    e2.tick()?;

    info!("Publishing data");

    let data = IotData::new(&[
        EpData::new(27.3.into()),
        EpData::new(1016.2.into()),
        EpData::new(59.6.into()),
    ])
    .unwrap();

    let sig = e2.publish(data, &[])?;

    // Tick to update publish state
    assert_eq!(e1.tick()?, EngineEvent::ReceivedData(e2.id(), sig));

    Ok(())
}
