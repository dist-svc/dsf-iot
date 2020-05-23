extern crate structopt;
use structopt::StructOpt;

extern crate futures;
use futures::prelude::*;

extern crate async_std;
use async_std::task;

extern crate humantime;

#[macro_use]
extern crate tracing;

extern crate tracing_subscriber;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use dsf_iot::{Command, IotClient, IotData, IotError, IotService, Options};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "DSF IoT Client",
    about = "Distributed Service Discovery (DSF) client, used for managing dsf-iot services"
)]
struct Config {
    #[structopt(subcommand)]
    cmd: Command,

    #[structopt(flatten)]
    client_options: Options,

    #[structopt(long = "log-level", default_value = "info")]
    /// Enable verbose logging
    level: LevelFilter,
}

fn main() {
    // Fetch arguments
    let opts = Config::from_args();

    let filter = EnvFilter::from_default_env().add_directive("async_std=warn".parse().unwrap());

    // Setup logging
    let _ = FmtSubscriber::builder()
        .with_max_level(opts.level.clone())
        .with_env_filter(filter)
        .try_init();

    info!("opts: {:?}", opts);

    let res: Result<(), IotError> = task::block_on(async {
        // Create client connector
        let mut c = match IotClient::new(&opts.client_options) {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "Error connecting to daemon on '{}': {:?}",
                    &opts.client_options.daemon_socket, e
                );
                return Err(e);
            }
        };

        // Execute commands
        match opts.cmd {
            Command::Create(o) => {
                let res = c.create(o).await?;
                info!("{:?}", res);
            }
            Command::Locate(o) => {
                let res = c.search(&o.id).await?;
                info!("{:?}", res);
            }
            Command::List(o) => {
                let res = c.list(o).await?;
                print_service_list(&res);
            }
            Command::Register(o) => {
                let res = c.register(o).await?;
                info!("{:?}", res);
            }
            Command::Publish(o) => {
                let res = c.publish(o).await?;
                info!("{:?}", res);
            }
            Command::Query(o) => {
                let (service, data) = c.query(o).await?;
                print_service_data(&service, &data);
            }
            Command::Subscribe(o) => {
                let mut res = c.subscribe(o).await?;

                for i in res.next().await {
                    info!("{:?}", i);
                }
            }
        }

        Ok(())
    });

    if let Err(e) = res {
        error!("{:?}", e);
    }
}

fn print_service_list(services: &[IotService]) {
    for s in services {
        println!("Service: {}", s.id);
        println!("Endpoints: ");
        for i in 0..s.endpoints.len() {
            let e = &s.endpoints[i];

            println!("  - {}: {:?} (metadata: {:?})", i, e.kind, e.meta);
        }
    }
}

fn print_service_data(service: &IotService, data: &[IotData]) {
    println!("Service: {}", service.id);
    println!("Data: ");

    for d in data {
        println!("Object {} (previous: {:?})", d.signature, d.previous);

        for i in 0..d.data.len() {
            let ep_info = &service.endpoints[i];
            let ep_data = &d.data[i];

            println!(
                "  - {:?}: {} {}",
                ep_info.kind,
                ep_data.value,
                ep_info.kind.unit()
            );
        }
    }
}
