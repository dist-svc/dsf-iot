
use structopt::StructOpt;

use humantime::Duration;

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
    period: Duration,

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
        (Kind::Temperature, Flags::R, vec![]).into(),
        (Kind::Pressure, Flags::R, vec![]).into(),
        (Kind::Humidity, Flags::R, vec![]).into(),
    ];

    // TODO: split service and engine setup better

    // Setup engine
    let mut e = match Engine::udp(descriptors, "127.0.0.1:0", store) {
        Ok(e) => e,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to configure engine: {:?}", e));
        }
    };

    println!("Using service: {:?}", e.id());

    // Connect to sensor
    let i2c_bus = I2cdev::new(&opts.i2c_dev).expect("error connecting to i2c bus");
    let mut bme280 = BME280::new(i2c_bus, opts.i2c_addr, Delay);
    bme280.init().unwrap();

    // Run sensor loop
    loop {
        // Take measurement
        let m = bme280.measure().unwrap();

        let data = [
            Data::new(m.temperature.into(), vec![]),
            Data::new((m.pressure / 1000.0).into(), vec![]),
            Data::new(m.humidity.into(), vec![]),
        ];

        println!("Measurement: {:?}", data);

        // Publish new object
        let (b, n) = (&data[..]).encode_buff::<512>()?;
        match e.publish(&b[..n], &[]) {
            Ok(_) => {
                println!("Published object: ")
            },
            Err(e) => {
                println!("Failed to publish object");
            }
        }

        // Wait until next measurement
        std::thread::sleep(*opts.period);
    }

    Ok(())
}
