use network_interface::Addr;
use std::net::IpAddr;
use tracing::instrument;

use tokio::net::TcpSocket;

#[instrument]
pub fn bind_socket(addr: IpAddr) -> std::io::Result<TcpSocket> {
    let socket = match addr {
        IpAddr::V4(_) => TcpSocket::new_v4()?,
        IpAddr::V6(_) => TcpSocket::new_v6()?,
    };

    socket.set_reuseaddr(true)?;
    socket.bind((addr, 0).into())?;

    Ok(socket)
}

pub fn get_valid_addresses(addresses: &[Addr]) -> Vec<IpAddr> {
    addresses
        .iter()
        .map(|addr| addr.ip())
        .filter(|addr| !is_local_address(addr))
        .filter(|addr| bind_socket(*addr).is_ok())
        .collect()
}

fn is_local_address(addr: &IpAddr) -> bool {
    if addr.is_loopback() {
        return true;
    }

    match addr {
        IpAddr::V4(ip) => {
            if ip.is_link_local() {
                return true;
            }
        }
        IpAddr::V6(ip) => {
            // Check for link-local (fe80::/10)
            if ip.segments()[0] & 0xffc0 == 0xfe80 {
                return true;
            }
        }
    }

    false
}
