use std::net::IpAddr;
use tracing::instrument;

use tokio::net::TcpSocket;

#[instrument]
pub fn bind_socket(addr: IpAddr, interface_name: String) -> std::io::Result<TcpSocket> {
    let socket = match addr {
        IpAddr::V4(_) => TcpSocket::new_v4()?,
        IpAddr::V6(_) => TcpSocket::new_v6()?,
    };

    socket.set_reuseaddr(true)?;
    socket.bind((addr, 0).into())?;
    if !interface_name.is_empty() {
        socket.bind_device(Some(interface_name.as_bytes())).unwrap();
    }

    Ok(socket)
}
