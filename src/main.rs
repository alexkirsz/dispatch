use std::{net::IpAddr, str::FromStr};

use debug::LogStrategy;
use dispatcher::WeightedAddress;
use eyre::Result;
use structopt::StructOpt;

mod debug;
mod dispatcher;
mod list;
mod net;
mod server;
mod socks;

#[derive(Debug, StructOpt)]
/// A proxy that balances traffic between multiple internet connections
struct Opt {
    /// Write debug logs to stdout instead of a file
    #[structopt(short, long)]
    debug: bool,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Lists all available network interfaces
    List,
    /// Starts the SOCKS proxy server
    Start {
        /// Which IP to accept connections from
        #[structopt(default_value = "127.0.0.1", long)]
        ip: IpAddr,
        /// Which port to listen to for connections
        #[structopt(default_value = "1080", long)]
        port: u16,
        /// The network interface IP addresses to dispatch to, in the form of <address>[@priority]
        #[structopt(required = true, parse(try_from_str = WeightedAddress::from_str))]
        addresses: Vec<WeightedAddress>,
    },
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

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
        } => server::server(ip, port, addresses)?,
    }

    Ok(())
}
