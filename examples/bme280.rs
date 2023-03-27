
use structopt::StructOpt;

use humantime::Duration;

use linux_embedded_hal::{self as hal, Delay, I2cdev};

use bme280::BME280;

use tracing::{debug, info, error};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_iot::prelude::*;

#[derive(Debug, StructOpt)]
#[structopt(name = "DSF IoT BME280 Client")]
struct Config {
    #[structopt(flatten)]
    service: ServiceIdentifier,

    #[structopt(flatten)]
    daemon_options: Options,

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

#[async_std::main]
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
    let mut c = IotClient::new(&opts.daemon_options)?;

    let service = opts.service.clone();

    let handle = match (&service.index, &service.id) {
        (None, None) => {
            println!("Creating new BME280 service");

            let s = c
                .create(CreateOptions {
                    endpoints: vec![
                        Descriptor::new(Kind::Temperature, Flags::R, &[]),
                        Descriptor::new(Kind::Pressure, Flags::R, &[]),
                        Descriptor::new(Kind::Humidity, Flags::R, &[]),
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
            Data::new(m.temperature.into(), &[]),
            Data::new((m.pressure / 1000.0).into(), &[]),
            Data::new(m.humidity.into(), &[]),
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
        tokio::task::sleep(*opts.period).await;
    }

    Ok(())
}
