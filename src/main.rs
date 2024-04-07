use std::{net::IpAddr, str::FromStr};

use clap::Parser;
use debug::LogStrategy;
use dispatcher::{RawWeightedAddress, WeightedAddress};
use eyre::Result;

mod debug;
mod dispatcher;
mod list;
mod net;
mod server;
mod socks;

/// A proxy that balances traffic between multiple internet connections
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Opt {
    /// Write debug logs to stdout instead of a file
    #[arg(short, long)]
    debug: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Lists all available network interfaces
    List,
    /// Starts the SOCKS proxy server
    Start {
        /// Which IP to accept connections from
        #[arg(default_value = "127.0.0.1", long)]
        ip: IpAddr,
        /// Which port to listen to for connections
        #[arg(default_value = "1080", long)]
        port: u16,
        /// The network interface IP addresses to dispatch to, in the form of <address>[/priority]
        #[arg(required = true, value_parser = RawWeightedAddress::from_str)]
        addresses: Vec<RawWeightedAddress>,
    },
}

fn main() -> Result<()> {
    let opt = Opt::parse();

    let _guard = debug::install(if opt.debug {
        LogStrategy::Stdout
    } else {
        LogStrategy::File
    })?;

    match opt.command {
        Command::List => list::list(),
        Command::Start {
            ip,
            port,
            addresses,
        } => {
            let addresses = WeightedAddress::resolve(addresses)?;
            server::server(ip, port, addresses)?
        }
    }

    Ok(())
}
