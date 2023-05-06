use std::{
    fmt::{Display, Formatter},
    net::{IpAddr, SocketAddr},
    num::NonZeroUsize,
    str::FromStr,
    sync::Arc,
};

use color_eyre::Help;
use eyre::Result;
use tokio::sync::Mutex;
use tracing::instrument;

use super::Dispatch;

#[derive(Clone, Debug)]
pub struct WeightedAddress {
    ip: IpAddr,
    weight: NonZeroUsize,
}

impl FromStr for WeightedAddress {
    type Err = eyre::Report;

    fn from_str(src: &str) -> Result<Self> {
        let mut items = src.split('@');

        let ip: IpAddr = items.next().unwrap().parse()?;

        let weight = match items.next() {
            Some(priority) => priority.parse()?,
            None => NonZeroUsize::new(1).unwrap(),
        };

        Ok(WeightedAddress { ip, weight })
    }
}

impl Display for WeightedAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.ip.fmt(f)?;
        f.write_fmt(format_args!("@{}", self.weight))?;
        Ok(())
    }
}

#[derive(Debug)]
struct WeightedRoundRobinDispatcherInner {
    ipv4: State,
    ipv6: State,
}

#[derive(Debug)]
struct State {
    addresses: Vec<WeightedAddress>,
    address_idx: usize,
    count: usize,
}

impl WeightedRoundRobinDispatcherInner {
    fn new(addresses: Vec<WeightedAddress>) -> WeightedRoundRobinDispatcherInner {
        debug_assert!(
            !addresses.is_empty(),
            "dispatcher should have at least one address"
        );

        // TODO: Use drain_filter once stable.
        let (ipv4_addresses, ipv6_addresses) =
            addresses.into_iter().partition(|addr| addr.ip.is_ipv4());

        WeightedRoundRobinDispatcherInner {
            ipv4: State {
                addresses: ipv4_addresses,
                address_idx: 0,
                count: 0,
            },
            ipv6: State {
                addresses: ipv6_addresses,
                address_idx: 0,
                count: 0,
            },
        }
    }

    fn dispatch(&mut self, remote_addr: &SocketAddr) -> Result<IpAddr> {
        let state = self.select_state(remote_addr)?;

        let address = &state.addresses[state.address_idx];
        let ip = address.ip;

        state.count += 1;
        if state.count == usize::from(address.weight) {
            state.count = 0;
            state.address_idx = (state.address_idx + 1) % state.addresses.len();
        }

        Ok(ip)
    }

    fn select_state(&mut self, remote_addr: &SocketAddr) -> Result<&mut State> {
        let state = match remote_addr.ip() {
            IpAddr::V4(_) => &mut self.ipv4,
            IpAddr::V6(_) => &mut self.ipv6,
        };

        if state.addresses.is_empty() {
            return Err(eyre::eyre!(
                "Address type mismatch: no configured local address or interface can connect to \
                remote address `{}` ({}) because the address types are incompatible",
                remote_addr,
                addr_type(remote_addr.ip())
            )
            .suggestion(format!(
                "Please ensure that the local addresses or network interfaces you have \
                configured support {}",
                addr_type(remote_addr.ip())
            ))
            .suggestion(
                "As a last resort, you can try to disable IPv6 support in the settings of your main \
                network interface to force your OS to use IPv4 everywhere",
            ));
        }

        Ok(state)
    }
}

#[derive(Debug, Clone)]
pub struct WeightedRoundRobinDispatcher(Arc<Mutex<WeightedRoundRobinDispatcherInner>>);

impl WeightedRoundRobinDispatcher {
    pub fn new(addresses: Vec<WeightedAddress>) -> WeightedRoundRobinDispatcher {
        WeightedRoundRobinDispatcher(Arc::new(Mutex::new(
            WeightedRoundRobinDispatcherInner::new(addresses),
        )))
    }
}

#[async_trait::async_trait]
impl Dispatch for WeightedRoundRobinDispatcher {
    #[instrument]
    async fn dispatch(&self, remote_addr: &SocketAddr) -> Result<IpAddr> {
        let mut dispatcher = self.0.lock().await;
        dispatcher.dispatch(remote_addr)
    }
}

fn addr_type(addr: IpAddr) -> &'static str {
    match addr {
        IpAddr::V4(_) => "IPv4",
        IpAddr::V6(_) => "IPv6",
    }
}
