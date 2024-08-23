use clap::Parser;

use humantime::Duration;

use linux_embedded_hal::{self as hal, Delay, I2cdev};

use blinkt::{Blinkt, BlinktSpi};

use tracing::{debug, error, info};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_iot::prelude::*;

#[derive(Debug, Parser)]
#[clap(name = "DSF IoT APA102 LED CLient")]
struct Args {
    #[clap(flatten)]
    service: ServiceIdentifier,

    #[clap(flatten)]
    daemon_options: Config,

    #[clap(long, default_value = "/dev/spidev1.0")]
    /// Specify the SPI port for the LEDs
    spi_dev: String,

    #[clap(short='n', long, default_value = " 8")]
    /// Number of LEDs to be driven
    led_count: usize,

    #[clap(long, default_value = "5s")]
    /// Specify a period for refreshing the service state
    period: Duration,

    #[clap(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Args::parse();

    let filter = EnvFilter::from_default_env()
        .add_directive(format!("dsf_iot={}", opts.log_level).parse().unwrap())
        .add_directive("async_std=warn".parse().unwrap());

    // Setup logging
    let _ = FmtSubscriber::builder().with_env_filter(filter).try_init();

    debug!("opts: {:?}", opts);

    // Create client connector
    let mut c = IotClient::new(opts.daemon_options).await?;

    let service = opts.service.clone();

    let handle = match (&service.index, &service.id) {
        (None, None) => {
            println!("Creating new APA102 LED service");

            let s = c
                .create(CreateOptions {
                    endpoints: vec![
                        EpDescriptor::new(EpKind::State, EpFlags::RW),
                        EpDescriptor::new(EpKind::Colour, EpFlags::RW),
                        EpDescriptor::new(EpKind::Brightness, EpFlags::RW),
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

    // Connect to LEDs
    // TODO: allow SPI configuration
    let blinkt_spi = BlinktSpi::default();
    let mut blinkt = Blinkt::with_spi(blinkt_spi, opts.led_count);

    // TODO: restore to previous state?

    // TODO: subscribe to incoming messages

    // Run control loop
    loop {
        // TODO: Await control message

        // TODO: publish state update

        // Update state
        for i in 0..opts.led_count {
            blinkt.set_pixel(i, 127, 0, 0);
        }
        blinkt.show()?;
    }

    Ok(())
}
