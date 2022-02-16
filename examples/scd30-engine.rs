
use dsf_iot::endpoint::DataRef;
use hal::i2cdev::linux::LinuxI2CError;
use structopt::StructOpt;

use humantime::Duration;

use linux_embedded_hal::{self as hal, Delay, I2cdev};
use embedded_hal::blocking::delay::DelayMs;

use sensor_scd30::{Scd30, Measurement};

use tracing::{debug, info, warn, error};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_core::options::Metadata;
use dsf_core::base::{Encode};

use dsf_iot::prelude::*;
use dsf_iot::engine::{Engine, SledStore};

#[derive(Debug, StructOpt)]
#[structopt(name = "DSF IoT BME280 Client")]
struct Config {
    #[structopt(long, default_value = "/dev/i2c-1")]
    /// Specify the I2C port for the sensor
    i2c_dev: String,

    #[structopt(long, default_value = "1m")]
    /// Specify a period for sensor readings
    period: Duration,

    #[structopt(long, default_value = "bme280.db")]
    /// Database file for BME280 engine
    database: String,

    #[structopt(long, default_value = "100ms")]
    /// Delay between sensor poll operations
    poll_delay: Duration,

    #[structopt(long = "allowed-errors", default_value="3")]
    /// Number of allowed I2C errors (per measurement attempt) prior to exiting
    pub allowed_errors: usize,

    #[structopt(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Config::from_args();

    let filter = EnvFilter::from_default_env()
        .add_directive("sled=warn".parse().unwrap())
        .add_directive("async_std=warn".parse().unwrap())
        .add_directive(opts.log_level.into());

    // Setup logging
    let _ = FmtSubscriber::builder().with_env_filter(filter).try_init();

    debug!("opts: {:?}", opts);

    // TODO: setup store
    let store = match SledStore::new(&opts.database) {
        Ok(e) => e,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to open store: {:?}", e));
        }
    };

    // Setup service
    let descriptors = [
        EpDescriptor::new(EpKind::Temperature, EpFlags::R, vec![]),
        EpDescriptor::new(EpKind::Co2, EpFlags::R, vec![]),
        EpDescriptor::new(EpKind::Humidity, EpFlags::R, vec![]),
    ];

    // TODO: split service and engine setup better

    // Setup engine
    let mut e = match Engine::udp(descriptors, "127.0.0.1:0", store) {
        Ok(e) => e,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to configure engine: {:?}", e));
        }
    };

    info!("Using service: {:?}", e.id());

    // Connect to sensor
    let i2c_bus = I2cdev::new(&opts.i2c_dev).expect("error connecting to i2c bus");
    let mut scd30 = match Scd30::new(i2c_bus, Delay{}) {
        Ok(d) => d,
        Err(e) => return Err(anyhow::anyhow!("Failed to connect to SCD30: {:?}", e)),
    };

    if let Err(e) = sensor_init(&opts, &mut scd30) {
        return Err(anyhow::anyhow!("Failed to start continuous mode: {:?}", e));
    }

    debug!("Waiting for sensor to initialise");
    std::thread::sleep(*opts.period);

    // Run sensor loop
    loop {
        debug!("Starting sensor read cycle");

        let m = match sensor_read(&opts, &mut scd30) {
            Ok(m) => m,
            Err(e) => {
                error!("Sensor read error: {:?}, attempting re-initialisation", e);
                
                if let Err(e) = sensor_init(&opts, &mut scd30) {
                    error!("Failed to reinitalise sensor: {:?}", e);
                }

                continue;
            }
        };

        let data = [
            DataRef::new(m.temp.into(), &[]),
            DataRef::new(m.co2.into(), &[]),
            DataRef::new(m.rh.into(), &[]),
        ];

        info!("Measurement: {:?}", data);

        // Publish new object
        let (b, n) = (&data[..]).encode_buff::<512>()?;
        match e.publish(&b[..n], &[]) {
            Ok(sig) => {
                println!("Published object: {}", sig);
            },
            Err(e) => {
                println!("Failed to publish object: {:?}", e);
            }
        }

        // Wait until next measurement
        std::thread::sleep(*opts.period);
    }

    Ok(())
}

fn sensor_init(opts: &Config, scd30: &mut Scd30<I2cdev, Delay, LinuxI2CError>) -> anyhow::Result<()> {
    debug!("Applying soft reset");

    if let Err(e) = scd30.soft_reset() {
        return Err(anyhow::anyhow!("Failed to soft reset device {:?}", e));
    }

    Delay{}.delay_ms(500u32);

    debug!("Starting continuous mode");

    if let Err(e) = scd30.start_continuous(opts.period.as_secs() as u16) {
        warn!("Failed to start continuous mode: {:?}", e);
    }

    Ok(())
}

fn sensor_read(opts: &Config, scd30: &mut Scd30<I2cdev, Delay, LinuxI2CError>) -> anyhow::Result<Measurement> {
    let mut ready = false;
    let mut errors = 0;

    // Poll for sensor ready
    for _i in 0..100 {
        match scd30.data_ready() {
            Ok(true) => {
                ready = true;
                break;
            },
            Ok(false) => {
                std::thread::sleep(*opts.poll_delay);
            },
            Err(e) => {
                warn!("Error polling for sensor ready: {:?}", e);
                errors += 1;
            }
        };

        if errors > opts.allowed_errors {
            return Err(anyhow::anyhow!("sensor ready failed"));
        }
    }

    debug!("Sensor data ready state: {:?}", ready);

    if !ready {
        warn!("Sensor data ready timed-out");

        std::thread::sleep(*opts.period);

        return Err(anyhow::anyhow!("sensor read timeout"));
    }

    // If we're ready, attempt to read the data
    for _i in 0..10 {
        match scd30.read_data() {
            Ok(m) => return Ok(m),
            Err(e) => {
                warn!("Error reading sensor data: {:?}", e);
                errors += 1;
            },
        }

        if errors > opts.allowed_errors {
            error!("Exceeded maximum allowed I2C errors");
            return Err(anyhow::anyhow!("read data failed"));
        }
    }

    return Err(anyhow::anyhow!("I2C reads failed"))
}