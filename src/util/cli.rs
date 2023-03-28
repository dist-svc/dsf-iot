

use std::time::{Instant, Duration};

use dsf_core::prelude::ServiceBuilder;

use dsf_iot::{prelude::*, IotEngine};
use dsf_iot::engine::{Engine, MemoryStore, EngineEvent};
use dsf_rpc::DataInfo;

use structopt::StructOpt;

use futures::prelude::*;

use tracing::{debug, info, error};

use tracing_subscriber::filter::{LevelFilter};
use tracing_subscriber::FmtSubscriber;


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

    #[structopt(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Config::from_args();

    // Setup logging
    let _ = FmtSubscriber::builder()
        .with_max_level(opts.log_level.clone())
        .try_init();

    debug!("opts: {:?}", opts);

    // Unconnected commands
    match &opts.cmd {
        Command::GenKeys => {
            println!("Generating keys: ");

            let (id, keys) = IotClient::generate().unwrap();

            println!("ID: {}", id);
            println!("Public key: {}", keys.pub_key.unwrap());
            println!("Private key: {}", keys.pri_key.unwrap());
            println!("Secret key: {}", keys.sec_key.unwrap());

            return Ok(());
        },
        Command::Encode(opts) => {
            IotClient::encode(opts)?;

            return Ok(())
        },
        Command::Decode(opts) => {
            IotClient::decode(opts)?;

            return Ok(())
        },
        Command::Discover(_opts) => {
            // Create transient service
            let mut engine = IotEngine::<_, _, 512>::udp(IotInfo::new(&[]).unwrap(), "0.0.0.0:10100", MemoryStore::new())?;

            info!("Starting discovery from: {}", engine.id());

            // Issue discover message
            let req_id = engine.discover(&[], &[])?;

            // Await responses
            let then = Instant::now();
            while Instant::now().duration_since(then) < Duration::from_secs(3) {
                let mut buff = [0u8; 512];

                if let EngineEvent::Discover(id) = engine.tick()? {
                    info!("Discovered service: {:?}", id);
                }

            }

            info!("Discovery complete");

            return Ok(())
        }
        _ => (),
    }

    // Create client connector
    let mut c = match IotClient::new(&opts.client_options).await {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Error connecting to daemon on '{}': {:?}",
                &opts.client_options.daemon_socket, e
            );
            return Err(e.into());
        }
    };

    // Execute commands
    match opts.cmd {
        Command::Create(o) => {
            let res = c.create(o).await?;
            info!("{:?}", res);
        }
        Command::Locate(o) => {
            let (_h, s) = c.search(&o.id).await?;
            println!("Located service");
            print_service_list(&[s]);

        }
        Command::Info(o) => {
            let res = c.info(o).await?;
            print_service_list(&[res]);
        }
        Command::List(o) => {
            let res = c.list(o).await?;
            print_service_list(&res);
        }
        Command::Register(o) => {
            let res = c.register(o).await?;
            println!("{:?}", res);
        }
        Command::Publish(o) => {
            let res = c.publish(o).await?;
            println!("{:?}", res);
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
        _ => unreachable!(),
    }

    Ok(())
 
}

fn print_service_list(services: &[IotService]) {
    for s in services {
        println!("Service ID: {}", s.id);
        println!("Endpoints: ");
        for i in 0..s.endpoints.len() {
            let e = &s.endpoints[i];

            println!("  - {:2}: {:16} in {:4}", i, e.kind, e.kind.unit());
        }
    }
}

fn print_service_data(service: &IotService, data: &[(DataInfo, IotData)]) {
    println!("Service ID: {}", service.id);
    println!("Data: ");

    for (i, d) in data {
        let sig = i.signature.to_string();
        let prev = i.previous.as_ref().map(|v| v.to_string() ).unwrap_or("none".to_string());

        println!("Object: {} index: {} (previous: {})", &sig[..16], i.index, prev);

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
