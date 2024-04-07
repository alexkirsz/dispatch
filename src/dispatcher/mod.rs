mod weighted_rr;

use std::net::{IpAddr, SocketAddr};

use eyre::Result;

pub use weighted_rr::{RawWeightedAddress, WeightedAddress, WeightedRoundRobinDispatcher};

#[async_trait::async_trait]
pub trait Dispatch {
    async fn dispatch(&self, remote_address: &SocketAddr) -> Result<IpAddr>;
}
