use dsf_core::prelude::MaybeEncrypted;

use dsf_iot::prelude::*;
use dsf_rpc::{DataInfo, ServiceInfo, RegisterInfo, NsRegisterInfo, NsSearchInfo};

use clap::Parser;

use futures::prelude::*;

use tracing::{debug, error, info};

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Parser)]
#[clap(
    name = "DSF IoT Client",
    about = "Distributed Service Discovery (DSF) client, used for managing dsf-iot services"
)]
struct Args {
    #[clap(subcommand)]
    cmd: Command,

    #[clap(flatten)]
    client_options: Config,

    #[clap(long, default_value = "info")]
    /// Enable verbose logging
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Fetch arguments
    let opts = Args::parse();

    // Setup logging
    let _ = FmtSubscriber::builder()
        .with_max_level(opts.log_level.clone())
        .try_init();

    debug!("opts: {:?}", opts);

    // Create client connector
    let mut c = match IotClient::new(opts.client_options.clone()).await {
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
        Command::Data(o) => {
            let (service, eps, data) = c.query(o).await?;
            print_service_data(&service, &eps, &data);
        }
        Command::Subscribe(o) => {
            let mut res = c.subscribe(o).await?;

            while let Some(i) = res.next().await {
                info!("{:?}", i);
            }
        }
        Command::Discover(o) => {
            let res = c.discover(o).await?;
            print_service_list(&res);
        }
        Command::NsRegister(o) => {
            let (r, s, d) = c.ns_register(o).await?;
            print_register_info(r, &s, &d);
        }
        Command::NsSearch(o) => {
            let (i, s) = c.ns_search(o).await?;
            print_search_info(i, &s);
        }
        Command::GenKeys => {
            let (id, k) = IotClient::generate()?;
            info!("id: {id}");
            info!("public key: {}", k.pub_key.unwrap());
            info!("private key: {}", k.pri_key.unwrap());
            info!("secret key: {}", k.sec_key.unwrap());
        }
        Command::Control(o) => {
            let _ = c.control(o).await?;
            // TODO: wait for update / get updated device state?
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn print_register_info(reg: NsRegisterInfo, s: &ServiceInfo, d: &DataInfo<Vec<EpDescriptor>>) {
    println!("Registered service with ns {:#}", reg.ns);

    if let Some(n) = reg.name {
        println!("Name: {n}");
    }

    print_service(s, d);

    println!("TIDs: ");
    for t in reg.tids {
        println!("  - {t:#}");
    }
}

fn print_search_info(reg: NsSearchInfo, services: &[(ServiceInfo, DataInfo<Vec<EpDescriptor>>)]) {
    println!("Search using ns {:#}", reg.ns);
    println!("TID: {:#}", reg.hash);
    println!("Matching services: ");

    print_service_list(services);
}

fn print_service(s: &ServiceInfo, d: &DataInfo<Vec<EpDescriptor>>) {
    println!("Service ID: {} (short: {})", s.id, s.short_id);

    println!("Primary page: {:#} (index: {})", d.signature, d.index);

    print!("Endpoints: ");
    match &d.body {
        MaybeEncrypted::Cleartext(eps) => {
            println!("");
            print_endpoints(&eps)
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

fn print_service_list(services: &[(ServiceInfo, DataInfo<Vec<EpDescriptor>>)]) {
    for (s, d) in services {
        print_service(s, d);
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
    println!("Service ID: {:#} (short: {})", service.id, service.short_id);
    println!("Primary page: {:#} (index: {})", desc.signature, desc.index);

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

    println!("Data ({} objects): ", data.len());
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
