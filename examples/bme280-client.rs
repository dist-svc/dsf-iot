
use clap::Parser;

use humantime::Duration;

use linux_embedded_hal::{self as hal, Delay, I2cdev};

use bme280::BME280;

use tracing::{debug, info, error};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_iot::prelude::*;

#[derive(Debug, Parser)]
#[clap(name = "DSF IoT BME280 Client")]
struct Config {
    #[clap(flatten)]
    service: ServiceIdentifier,

    #[clap(flatten)]
    daemon_options: Options,

    #[clap(long, default_value = "/dev/i2c-1")]
    /// Specify the I2C port for the sensor
    i2c_dev: String,

    #[clap(long, default_value = "119")]
    /// Specify the I2C address for the sensor
    i2c_addr: u8,

    #[clap(long, default_value = "1m")]
    /// Specify a period for sensor readings
    period: Duration,

    #[clap(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Config::from_args();

    let filter = EnvFilter::from_default_env()
        .add_directive(format!("dsf_iot={}", opts.log_level).parse().unwrap())
        .add_directive("async_std=warn".parse().unwrap());

    // Setup logging
    let _ = FmtSubscriber::builder().with_env_filter(filter).try_init();

    debug!("opts: {:?}", opts);

    // Create client connector
    println!(
        "Connecting to client socket: '{}'",
        &opts.daemon_options.daemon_socket
    );
    let mut c = IotClient::new(&opts.daemon_options).await?;

    let service = opts.service.clone();

    let handle = match (&service.index, &service.id) {
        (None, None) => {
            println!("Creating new BME280 service");

            let s = c
                .create(CreateOptions {
                    endpoints: vec![
                        EpDescriptor::new(EpKind::Temperature, EpFlags::R),
                        EpDescriptor::new(EpKind::Pressure, EpFlags::R),
                        EpDescriptor::new(EpKind::Humidity, EpFlags::R),
                    ],
                    ..Default::default()
                })
                .await?;
            s
        }

        _ => {
            println!("Connecting to existing service");
            let s = c.base().info(service.into()).await?;

            println!("Located service: {:?}", s.1);

            s.0
        }
    };

    println!("Using service: {:?}", handle.id);

    // Connect to sensor
    let i2c_bus = I2cdev::new(&opts.i2c_dev).expect("error connecting to i2c bus");
    let mut bme280 = BME280::new(i2c_bus, opts.i2c_addr, Delay);
    bme280.init().unwrap();

    // Run sensor loop
    loop {
        // Take measurement
        let m = bme280.measure().unwrap();

        let data = vec![
            EpData::new(m.temperature.into()),
            EpData::new((m.pressure / 1000.0).into()),
            EpData::new(m.humidity.into()),
        ];

        println!("Measurement: {:?}", data);

        // Publish new object
        c.publish(PublishOptions {
            service: ServiceIdentifier::id(handle.id.clone()),
            data,
            meta: vec![],
        })
        .await?;

        // Wait until next measurement
        tokio::time::sleep(*opts.period).await;
    }

    Ok(())
}
