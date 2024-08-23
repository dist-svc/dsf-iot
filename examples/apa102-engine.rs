use std::time::{Instant, Duration};

use clap::Parser;
use humantime::{Duration as HumanDuration};

use tracing::{debug, error, info};
use tracing_subscriber::{filter::{EnvFilter, LevelFilter}, FmtSubscriber};

use blinkt::{Blinkt, BlinktSpi};

use dsf_core::prelude::Options;
use dsf_engine::{engine::EngineEvent, store::SqliteStore};
use dsf_iot::{endpoint::EpValue, prelude::*};



#[derive(Debug, Parser)]
#[clap(name = "DSF IoT BME280 Client")]
struct Config {
    #[clap(long, default_value = "/dev/spidev1.0")]
    /// Specify the SPI port for the LEDs
    spi_dev: String,

    #[clap(short='n', long, default_value = "8")]
    /// Number of LEDs to be driven
    led_count: usize,

    #[clap(long, default_value = "apa102.db")]
    /// Database file for APA102 engine
    database: String,

    #[clap(long, default_value = "5s")]
    /// Specify a period for refreshing the service state
    period: HumanDuration,

    #[clap(long)]
    /// Service name
    name: Option<String>,

    #[clap(long)]
    /// Service room
    room: Option<String>,

    #[clap(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Config::parse();

    let filter = EnvFilter::from_default_env()
        .add_directive("async_std=warn".parse().unwrap())
        .add_directive(opts.log_level.into());

    // Setup logging
    let _ = FmtSubscriber::builder().with_env_filter(filter).try_init();

    debug!("opts: {:?}", opts);

    // TODO: setup store
    let store = SqliteStore::new(&opts.database)?;

    // Setup service
    let descriptors = IotInfo::new(&[
        EpDescriptor::new(EpKind::State, EpFlags::RW),
        EpDescriptor::new(EpKind::Colour, EpFlags::RW),
        EpDescriptor::new(EpKind::Brightness, EpFlags::RW),
    ])
    .unwrap();

    let mut options = vec![];
    if let Some(v) = &opts.name {
        options.push(Options::name(v));
    }
    if let Some(v) = &opts.room {
        options.push(Options::room(v));
    }

    // TODO: split service and engine setup better

    // Setup engine
    let mut engine = match IotEngine::<_, _, 512>::udp(descriptors, &options, "0.0.0.0:10100", store)
    {
        Ok(e) => e,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to configure engine: {:?}", e));
        }
    };

    info!("Using service: {:?}", engine.id());

    // Connect to LEDs
    // TODO: allow SPI configuration
    let blinkt_spi = BlinktSpi::default();
    let mut blinkt = Blinkt::with_spi(blinkt_spi, opts.led_count);

    let mut last = Instant::now();

    let mut colour = (0xffu8, 0xffu8, 0xffu8);
    let mut brightness = 100;
    let mut state = true;

    // Initialise light state
    let (r, g, b) = mix_srgbb(state, colour, brightness);
    for i in 0..opts.led_count {
        blinkt.set_pixel(i, r, g, b);
    }
    blinkt.show()?;

    // Write initial state object
    let data = IotData::new(&[
        EpData::new(EpValue::Bool(state)),
        EpData::new(EpValue::Rgb(colour.0, colour.1, colour.2)),
        EpData::new(EpValue::Int32(brightness)),
    ])
    .unwrap();

    // Publish new object (including control ID)
    match engine.publish(data, &[]) {
        Ok(sig) => {
            info!("Published object: {:#}", sig);
        }
        Err(e) => {
            info!("Failed to publish object: {:?}", e);
        }
    }

    // Run update loop
    loop {
        let mut updated_by = None;

        // Tick engine to handle received messages etc.
        match engine.tick() {
            // Handle control events
            Ok(EngineEvent::Control(id, control)) => {
                info!("Received control message from {id} data: {control:?}");

                // Update control values
                // TODO: this -should- use matching against configured endpoints...
                for d in control.data {
                    match d.value {
                        EpValue::Bool(v) => state = v,
                        EpValue::Rgb(r, g, b) => colour = (r, g, b),
                        EpValue::Int32(b) => brightness = b,
                        _ => (),
                    }
                }

                updated_by = Some(id.clone());
            }
            Ok(_evt) => (),
            Err(e) => error!("Tick error: {:?}", e),
        };

        // Build and publish updated state object
        if let Some(id) = updated_by {
            let data = IotData::new(&[
                EpData::new(EpValue::Bool(state)),
                EpData::new(EpValue::Rgb(colour.0, colour.1, colour.2)),
                EpData::new(EpValue::Int32(brightness)),
            ])
            .unwrap();

            // Publish new object (including control ID)
            match engine.publish(data, &[Options::peer_id(id)]) {
                Ok(sig) => {
                    info!("Published object: {:#}", sig);
                }
                Err(e) => {
                    info!("Failed to publish object: {:?}", e);
                }
            }

            // Update light state to match
            // (brightness / 10 to avoid blinding nearby grad students)
            let (r, g, b) = mix_srgbb(state, colour, brightness / 10);
            for i in 0..opts.led_count {
                blinkt.set_pixel(i, r, g, b);
            }
            blinkt.show()?;
        }

        // If we're not yet due for an update, sleep and continue
        let now = Instant::now();
        if now.duration_since(last) < *opts.period {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Update last timestamp
        last = now;
    }
}

/// Mix state, RGB, and brightness into one value
// TODO: use a perceptual mixing function instead? via HSV?
fn mix_srgbb(state: bool, colour: (u8, u8, u8), brightness: i32) -> (u8, u8, u8) {
    if state == false {
        return (0, 0, 0)
    }

    (
        ((colour.0 as i32) * 100 / brightness) as u8,
        ((colour.1 as i32) * 100 / brightness) as u8,
        ((colour.2 as i32) * 100 / brightness) as u8
    )
}
