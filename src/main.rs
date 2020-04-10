
extern crate structopt;
use structopt::StructOpt;

extern crate futures;
use futures::prelude::*;

extern crate async_std;
use async_std::task;

extern crate humantime;
use humantime::Duration;

#[macro_use]
extern crate tracing;

extern crate tracing_subscriber;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::filter::LevelFilter;

use dsf_iot::{IotClient, Command, Error};

#[derive(Debug, StructOpt)]
#[structopt(name = "DSF IoT Client", about = "Distributed Service Discovery (DSF) client, used for managing dsf-iot services")]
struct Config {
    #[structopt(subcommand)]
    cmd: Command,

    #[structopt(short = "d", long = "daemon-socket", default_value = "/tmp/dsf.sock", env="DSF_SOCK")]
    /// Specify the socket to bind the DSF daemon
    daemon_socket: String,

    #[structopt(long = "log-level", default_value = "info")]
    /// Enable verbose logging
    level: LevelFilter,

    #[structopt(long, default_value = "3s")]
    /// Timeout for daemon requests
    timeout: Duration,
}

fn main() {
    // Fetch arguments
    let opts = Config::from_args();

    // Setup logging
    let _ = FmtSubscriber::builder().with_max_level(opts.level.clone()).try_init();

    info!("opts: {:?}", opts);

    let res: Result<(), Error> = task::block_on(async {

        // Create client connector
        debug!("Connecting to client socket: '{}'", &opts.daemon_socket);
        let mut c = match IotClient::new(&opts.daemon_socket, *opts.timeout) {
            Ok(c) => c,
            Err(e) => {
                error!("Error connecting to daemon on '{}': {:?}", &opts.daemon_socket, e);
                return Err(e)
            }
        };

        // Execute commands
        match opts.cmd {
            Command::Create(o) => {
                let res = c.create(o).await?;
                info!("{:?}", res);
            },
            Command::Locate(o) => {
                let res = c.search(&o.id).await?;
                info!("{:?}", res);
            },
            Command::List(o) => {
                let res = c.list(o).await?;
                info!("{:?}", res);
            },
            Command::Register(o) => {
                let res = c.register(o).await?;
                info!("{:?}", res);
            },
            Command::Publish(o) => {
                let res = c.publish(o).await?;
                info!("{:?}", res);
            },
            Command::Query(o) => {
                let res = c.query(o).await?;
                info!("{:?}", res);
            },
            Command::Subscribe(o) => {
                let mut res = c.subscribe(o).await?;

                for i in res.next().await {
                    info!("{:?}", i);
                }
                
            },
        }

        Ok(())
    });

    if let Err(e) = res {
        error!("{:?}", e);
    }
}
