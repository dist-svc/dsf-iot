use std::time::{Duration, Instant};

use clap::Parser;
use dsf_core::prelude::Options;

use embedded_hal::delay::DelayNs;
use linux_embedded_hal::{Delay, I2CError, I2cdev};

use sensor_scd30::{Measurement, Scd30};

use tracing::{debug, error, info, warn};
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

    #[clap(long, default_value = "1m")]
    /// Specify a period for sensor readings
    period: humantime::Duration,

    #[clap(long, default_value = "scd30.db")]
    /// Database file for SCD30 engine
    database: String,

    #[clap(long, default_value = "100ms")]
    /// Delay between sensor poll operations
    poll_delay: humantime::Duration,

    #[clap(long)]
    /// Service name
    name: Option<String>,

    #[clap(long)]
    /// Service room
    room: Option<String>,

    #[clap(long = "allowed-errors", default_value = "3")]
    /// Number of allowed I2C errors (per measurement attempt) prior to exiting
    pub allowed_errors: usize,

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

    // Setup store / database
    let store = match SqliteStore::new(&opts.database) {
        Ok(e) => e,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to open store: {:?}", e));
        }
    };

    // Setup service
    let descriptors = IotInfo::new(&[
        EpDescriptor::new(EpKind::Temperature, EpFlags::R),
        EpDescriptor::new(EpKind::Co2, EpFlags::R),
        EpDescriptor::new(EpKind::Humidity, EpFlags::R),
    ])
    .map_err(|_| anyhow::anyhow!("Descriptor allocation failed"))?;

    let mut options = vec![];
    if let Some(v) = &opts.name {
        options.push(Options::name(v));
    }
    if let Some(v) = &opts.room {
        options.push(Options::room(v));
    }

    // TODO: split service and engine setup better

    // Setup engine
    let mut engine =
        match IotEngine::<_, _, 512>::udp(descriptors, &options, "0.0.0.0:10100", store) {
            Ok(e) => e,
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to configure engine: {:?}", e));
            }
        };

    info!("Using service: {:?}", engine.id());
    //info!("Endpoints: {:?}", descriptors);

    // Connect to sensor
    let i2c_bus = I2cdev::new(&opts.i2c_dev).expect("error connecting to i2c bus");
    let mut scd30 = match Scd30::new(i2c_bus, Delay {}) {
        Ok(d) => d,
        Err(e) => return Err(anyhow::anyhow!("Failed to connect to SCD30: {:?}", e)),
    };

    if let Err(e) = sensor_init(&opts, &mut scd30) {
        return Err(anyhow::anyhow!("Failed to start continuous mode: {:?}", e));
    }

    // Run sensor loop
    let mut last = Instant::now();
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

        debug!("Starting sensor read cycle");

        // Otherwise, let's get reading!
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

        // Save the new measurement
        let data = IotData::new(&[
            EpData::new(m.temp.into()),
            EpData::new(m.co2.into()),
            EpData::new(m.rh.into()),
        ])
        .map_err(|_| anyhow::anyhow!("Data allocation failed"))?;

        info!("Measurement: {:?}", data);

        // Publish new object
        match engine.publish(data, &[]) {
            Ok(sig) => {
                println!("Published object: {:#}", sig);
            }
            Err(e) => {
                println!("Failed to publish object: {:?}", e);
            }
        }

        // Update timeout for next measurement
        last = now;
    }

    Ok(())
}

fn sensor_init(
    opts: &Config,
    scd30: &mut Scd30<I2cdev, Delay, I2CError>,
) -> anyhow::Result<()> {
    debug!("Applying soft reset");

    if let Err(e) = scd30.soft_reset() {
        return Err(anyhow::anyhow!("Failed to soft reset device {:?}", e));
    }

    Delay {}.delay_ms(500u32);

    debug!("Starting continuous mode");

    if let Err(e) = scd30.start_continuous(opts.period.as_secs() as u16) {
        warn!("Failed to start continuous mode: {:?}", e);
    }

    Ok(())
}

fn sensor_read(
    opts: &Config,
    scd30: &mut Scd30<I2cdev, Delay, I2CError>,
) -> anyhow::Result<Measurement> {
    let mut ready = false;
    let mut errors = 0;

    // Poll for sensor ready
    for _i in 0..100 {
        match scd30.data_ready() {
            Ok(true) => {
                ready = true;
                break;
            }
            Ok(false) => {
                std::thread::sleep(*opts.poll_delay);
            }
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
            }
        }

        if errors > opts.allowed_errors {
            error!("Exceeded maximum allowed I2C errors");
            return Err(anyhow::anyhow!("read data failed"));
        }
    }

    return Err(anyhow::anyhow!("I2C reads failed"));
}
