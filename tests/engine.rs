use std::net::{UdpSocket, SocketAddr};

use log::info;

use dsf_core::prelude::*;
use dsf_iot::prelude::*;

use dsf_iot::engine::{Engine, MemoryStore, EngineEvent};

type E = Engine<'static, UdpSocket, Vec<EpDescriptor>, MemoryStore, 512>;

fn new_engine(addr: &str, descriptors: Vec<EpDescriptor>) -> anyhow::Result<E> {
    
    // Create peer for sending requests
    let p = ServiceBuilder::generic().build()?;

    // Setup memory store with pre-filled peer keys
    let mut s = MemoryStore::<SocketAddr>::new();
    s.update(&p.id(), |k| *k = p.keys());

    // Setup engine with newly created service
    let e = E::udp(descriptors, addr, s)?;

    Ok(e)
}

#[test]
fn integration() -> anyhow::Result<()> {
    // Setup debug logging
    let log_cfg = simplelog::ConfigBuilder::new().add_filter_ignore_str("dsf_core::wire").build();
    let _ =
        simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, log_cfg);

    // Setup descriptors
    let descriptors = vec![
        EpDescriptor::new(EpKind::Temperature, EpFlags::R, vec![]),
        EpDescriptor::new(EpKind::Pressure, EpFlags::R, vec![]),
        EpDescriptor::new(EpKind::Humidity, EpFlags::R, vec![]),
    ];

    // Create a pair of engines
    let mut e1 = new_engine("127.0.0.1:11000", descriptors.clone())?;
    let mut e2 = new_engine("127.0.0.2:11000", descriptors.clone())?;


    // Tick engines to get started
    e1.tick()?;
    e2.tick()?;


    info!("Attempting discovery");

    // Attempt local service discovery
    let ep_filter: &[EpDescriptor] = &[EpDescriptor::new(EpKind::Temperature, EpFlags::R, vec![])];
    let (body, n) = ep_filter.encode_buff::<128>().unwrap();
    e1.discover(&body[..n], &[])?;

    // Tick to update discovery state
    e1.tick()?;
    e2.tick()?;

    e1.tick()?;
    e2.tick()?;


    info!("Starting subscribe");

    // Attempt subscription
    e1.subscribe(e2.id(), e2.addr()?)?;

    // Tick to update discovery state
    assert_eq!(e2.tick()?, EngineEvent::SubscribeFrom(e1.id()));
    assert_eq!(e1.tick()?, EngineEvent::SubscribedTo(e2.id()));

    e2.tick()?;


    info!("Publishing data");

    let data = IotData::<stor::Const<0>, _>::new([
        EpData::new(27.3.into(), []),
        EpData::new(1016.2.into(), []),
        EpData::new(59.6.into(), []),
    ]);
    let mut data_buff = [0u8; 128];
    let n = data.encode(&mut data_buff).unwrap();

    e2.publish(&data_buff[..n], &[])?;

    // Tick to update publish state
    assert_eq!(e1.tick()?, EngineEvent::ReceivedData(e2.id()));

    Ok(())
}
