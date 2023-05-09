use std::time::{Duration, Instant};

use dsf_core::prelude::{MaybeEncrypted, ServiceBuilder};

use dsf_engine::{
    engine::{Engine, EngineEvent},
    store::MemoryStore,
};
use dsf_iot::{prelude::*, IotEngine};
use dsf_rpc::{DataInfo, ServiceInfo};

use clap::Parser;

use futures::prelude::*;

use tracing::{debug, error, info};

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::FmtSubscriber;

const BUFF_LEN: usize = 2048;

#[derive(Debug, Parser)]
#[clap(
    name = "DSF IoT Client",
    about = "Distributed Service Discovery (DSF) client, used for managing dsf-iot services"
)]
struct Config {
    #[clap(subcommand)]
    cmd: Command,

    #[clap(flatten)]
    client_options: Options,

    #[clap(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Config::parse();

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
        }
        Command::Discover(_opts) => {
            let port = portpicker::pick_unused_port().unwrap();

            // Create transient service
            let mut engine = IotEngine::<_, _, BUFF_LEN>::udp(
                IotInfo::new(&[]).unwrap(),
                format!("0.0.0.0:{}", port),
                MemoryStore::new(),
            )?;

            info!("Starting discovery from: {}", engine.id());

            // Issue discover message
            let req_id = engine.discover(&[], &[])?;

            // Await responses
            let then = Instant::now();
            while Instant::now().duration_since(then) < Duration::from_secs(3) {
                if let EngineEvent::Discover(id) = engine.tick()? {
                    info!("Discovered service: {:?}", id);
                }
            }

            info!("Discovery complete");

            return Ok(());
        }
        _ => (),
    }

    // Create client connector
    let mut c = match IotClient::new(&opts.client_options).await {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Error connecting to daemon on '{}': {:?}",
                &opts.client_options.daemon_socket(),
                e
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
            let (_h, i, e) = c.search(&o.id).await?;
            println!("Located service");
            print_service_list(&[(i, e)]);
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
            let (service, eps, data) = c.query(o).await?;
            print_service_data(&service, &eps, &data);
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

fn print_service_list(services: &[(ServiceInfo, DataInfo<Vec<EpDescriptor>>)]) {
    for (s, d) in services {
        println!("Service ID: {:#}", s.id);
        print!("Endpoints: ");
        match &d.body {
            MaybeEncrypted::Cleartext(eps) => {
                println!("");
                print_endpoints(&eps)
            }
            MaybeEncrypted::Encrypted(_) => println!("ENCRYPTED"),
            MaybeEncrypted::None => println!("None"),
        }
    }
}

fn print_endpoints(eps: &[EpDescriptor]) {
    for (i, e) in eps.iter().enumerate() {
        println!("  - {:2}: {:13} in {:4}", i, e.kind, e.kind.unit());
    }
}

fn print_service_data(
    service: &ServiceInfo,
    desc: &DataInfo<Vec<EpDescriptor>>,
    data: &[DataInfo<Vec<EpData>>],
) {
    println!("Service ID: {:#}", service.id);
    println!("Primary page: {:#}", desc.signature);

    print!("Endpoints: ");
    let endpoints = match &desc.body {
        MaybeEncrypted::Cleartext(eps) => {
            println!("");
            print_endpoints(&eps);
            eps
        }
        _ => {
            error!("Cannot print data for private service without decryption");
            return;
        }
    };

    match &desc.private_options {
        MaybeEncrypted::Cleartext(options) if options.len() > 0 => {
            println!("  private_options: ");
            for o in options {
                println!("    - {o:#}");
            }
        }
        MaybeEncrypted::Encrypted(_) => println!("  private_options: Encrypted"),
        _ => (),
    };

    print!("  public_options: ");
    if desc.public_options.len() == 0 {
        println!("Empty")
    } else {
        println!("");
        for o in &desc.public_options {
            println!("    - {o:#}");
        }
    }

    println!("Data: ");
    for d in data {
        println!("Object: {:#} index: {}", d.signature, d.index);

        print!("  values: ");
        match &d.body {
            MaybeEncrypted::Cleartext(data) => {
                println!("");
                for (i, d) in data.iter().enumerate() {
                    println!(
                        "    - {:16}: {:6} {}",
                        endpoints[i].kind,
                        d.value,
                        endpoints[i].kind.unit()
                    );
                }
            }
            MaybeEncrypted::Encrypted(_) => println!("ENCRYPTED"),
            MaybeEncrypted::None => println!("None"),
        }

        match &d.private_options {
            MaybeEncrypted::Cleartext(options) if options.len() > 0 => {
                println!("  private_options: ");
                for o in options {
                    println!("    - {o:#}");
                }
            }
            MaybeEncrypted::Encrypted(_) => println!("  private_options: Encrypted"),
            _ => (),
        };

        print!("  public_options: ");
        if d.public_options.len() == 0 {
            println!("Empty")
        } else {
            println!("");
            for o in &d.public_options {
                println!("    - {o:#}");
            }
        }
    }
}
