
use std::time::{Instant, Duration};

use dsf_iot::endpoint::DataRef;
use structopt::StructOpt;

use linux_embedded_hal::{self as hal, Delay, I2cdev};

use bme280::BME280;

use tracing::{debug, info, error};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_core::options::Metadata;
use dsf_core::base::{Encode};

use dsf_iot::prelude::*;
use dsf_iot::engine::{Engine, MemoryStore};

#[derive(Debug, StructOpt)]
#[structopt(name = "DSF IoT BME280 Client")]
struct Config {
    #[structopt(long, default_value = "/dev/i2c-1")]
    /// Specify the I2C port for the sensor
    i2c_dev: String,

    #[structopt(long, default_value = "119")]
    /// Specify the I2C address for the sensor
    i2c_addr: u8,

    #[structopt(long, default_value = "1m")]
    /// Specify a period for sensor readings
    period: humantime::Duration,

    #[structopt(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Config::from_args();

    let filter = EnvFilter::from_default_env()
        .add_directive("async_std=warn".parse().unwrap())
        .add_directive(opts.log_level.into());

    // Setup logging
    let _ = FmtSubscriber::builder().with_env_filter(filter).try_init();

    debug!("opts: {:?}", opts);

    // TODO: setup store
    let store = MemoryStore::new();

    // Setup service
    let descriptors = [
        EpDescriptor::new(EpKind::Temperature, EpFlags::R, vec![]),
        EpDescriptor::new(EpKind::Pressure, EpFlags::R, vec![]),
        EpDescriptor::new(EpKind::Humidity, EpFlags::R, vec![]),
    ];

    // TODO: split service and engine setup better

    // Setup engine
    let mut engine = match Engine::udp(descriptors, "127.0.0.1:0", store) {
        Ok(e) => e,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to configure engine: {:?}", e));
        }
    };

    println!("Using service: {:?}", engine.id());

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

        let data = [
            DataRef::new(m.temperature.into(), &[]),
            DataRef::new((m.pressure / 1000.0).into(), &[]),
            DataRef::new(m.humidity.into(), &[]),
        ];

        println!("Measurement: {:?}", data);

        // Publish new object
        let (b, n) = (&data[..]).encode_buff::<512>()?;
        match engine.publish(&b[..n], &[]) {
            Ok(_) => {
                println!("Published object: ")
            },
            Err(e) => {
                println!("Failed to publish object");
            }
        }

        // Update last timestamp
        last = now;
    }
}
