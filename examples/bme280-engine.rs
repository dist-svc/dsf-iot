use std::time::{Duration, Instant};

use clap::Parser;

use dsf_core::prelude::Options;
use linux_embedded_hal::{Delay, I2cdev};

use bme280::BME280;

use tracing::{debug, error, info};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_engine::store::SqliteStore;
use dsf_iot::prelude::*;

#[derive(Debug, Parser)]
#[clap(name = "DSF IoT BME280 Client")]
struct Config {
    #[clap(long, default_value = "/dev/i2c-1")]
    /// Specify the I2C port for the sensor
    i2c_dev: String,

    #[clap(long, default_value = "119")]
    /// Specify the I2C address for the sensor
    i2c_addr: u8,

    #[clap(long, default_value = "bme280.db")]
    /// Database file for BME280 engine
    database: String,

    #[clap(long, default_value = "1m")]
    /// Specify a period for sensor readings
    period: humantime::Duration,

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
        EpDescriptor::new(EpKind::Temperature, EpFlags::R),
        EpDescriptor::new(EpKind::Pressure, EpFlags::R),
        EpDescriptor::new(EpKind::Humidity, EpFlags::R),
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

    // Connect to sensor
    let i2c_bus = I2cdev::new(&opts.i2c_dev).expect("error connecting to i2c bus");
    let mut bme280 = BME280::new(i2c_bus, opts.i2c_addr, Delay);
    bme280.init().unwrap();

    let mut last = Instant::now();

    // Run sensor loop
    loop {
        // Tick engine to handle received messages etc.
        if let Err(e) = engine.tick() {
            error!("Tick error: {:?}", e);
        }

        // If we're not yet due for a measurement, sleep and continue
        let now = Instant::now();
        if now.duration_since(last) < *opts.period {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        // When we've timed out, take measurement
        let m = bme280.measure().unwrap();

        let data = IotData::new(&[
            EpData::new(m.temperature.into()),
            EpData::new((m.pressure / 1000.0).into()),
            EpData::new(m.humidity.into()),
        ])
        .unwrap();

        println!("Measurement: {:?}", data);

        // Publish new object
        match engine.publish(data, &[]) {
            Ok(sig) => {
                println!("Published object: {:#}", sig);
            }
            Err(e) => {
                println!("Failed to publish object: {:?}", e);
            }
        }

        // Update last timestamp
        last = now;
    }
}
