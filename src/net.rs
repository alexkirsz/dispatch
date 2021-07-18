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
