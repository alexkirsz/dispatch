use std::{
    fmt::Debug,
    net::{IpAddr, SocketAddr},
};

use color_eyre::Section;
use eyre::{eyre, Report, Result, WrapErr};
use socksv5::{
    v4::SocksV4Command,
    v5::{SocksV5Command, SocksV5Handshake},
    SocksVersion, SocksVersionError,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    net::{lookup_host, TcpSocket, TcpStream, ToSocketAddrs},
};
use tracing::instrument;

use crate::{dispatcher::Dispatch, net::bind_socket};

const HTTP_METHODS: [&'static str; 9] = [
    "GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH",
];

#[instrument]
fn assert_supports_noauth(handshake: &SocksV5Handshake) -> Result<()> {
    if let None = handshake
        .methods
        .iter()
        .find(|m| **m == socksv5::v5::SocksV5AuthMethod::Noauth)
    {
        Err(unsupported_auth_error())
    } else {
        Ok(())
    }
}

#[instrument]
fn try_bind_socket(addr: IpAddr) -> Result<TcpSocket> {
    bind_socket(addr).map_err(|err| match err.raw_os_error() {
        // Can't assign requested address
        Some(49) => eyre::eyre!(err).wrap_err(unaccessible_local_address_error(&addr)),
        _ => eyre::eyre!(err),
    })
}

#[instrument]
async fn lookup<T>(host: T) -> Result<SocketAddr>
where
    T: ToSocketAddrs + Debug,
{
    let addr = lookup_host(&host)
        .await
        .map_err(|err| eyre::eyre!(err).wrap_err(resolve_host_error(&host)))?
        .next()
        .ok_or_else(|| resolve_host_error(&host))?;
    Ok(addr)
}

#[derive(Debug)]
pub struct SocksHandshake<R, W, D>
where
    R: AsyncRead + Unpin + Debug,
    W: AsyncWrite + Unpin + Debug,
    D: Dispatch + Debug,
{
    reader: R,
    writer: W,
    dispatcher: D,
}

impl<R, W, D> SocksHandshake<R, W, D>
where
    R: AsyncRead + Unpin + Debug,
    W: AsyncWrite + Unpin + Debug,
    D: Dispatch + Debug,
{
    pub fn new(reader: R, writer: W, dispatcher: D) -> SocksHandshake<R, W, D> {
        SocksHandshake {
            reader,
            writer,
            dispatcher,
        }
    }

    pub async fn handshake(&mut self) -> Result<TcpStream> {
        match socksv5::read_version(&mut self.reader).await {
            Err(err) => Err(self.handle_version_error(err).await),
            Ok(version) => self.handle_handshake_with_version(version).await,
        }
    }

    #[instrument]
    async fn handle_version_error(&mut self, err: SocksVersionError) -> eyre::Report {
        match err {
            SocksVersionError::InvalidVersion(byte) => {
                match byte as char {
                    // HTTP method prefixes
                    'C' | 'G' | 'P' | 'H' | 'D' | 'O' | 'T' => {
                        let mut out = [0u8; 1024];
                        out[0] = byte;
                        match self.reader.read(&mut out[1..]).await {
                            Ok(read) => {
                                let out = String::from_utf8_lossy(&out[..read + 1]);
                                if HTTP_METHODS.iter().any(|method| out.starts_with(method)) {
                                    http_header_error(&out)
                                } else {
                                    err.into()
                                }
                            }
                            Err(read_err) => eyre!(err).wrap_err(read_err),
                        }
                    }
                    _ => err.into(),
                }
            }
            err => err.into(),
        }
    }

    #[instrument]
    async fn handle_handshake_with_version(&mut self, version: SocksVersion) -> Result<TcpStream> {
        match version {
            socksv5::SocksVersion::V5 => {
                let handshake = socksv5::v5::read_handshake_skip_version(&mut self.reader).await?;

                self.handle_auth(&handshake).await?;

                let host = self.handle_request_v5().await?;

                let local_addr = self
                    .dispatcher
                    .dispatch(&host)
                    .await
                    .wrap_err_with(dispatch_error)?;

                self.handle_connect_v5(host, local_addr).await
            }
            socksv5::SocksVersion::V4 => {
                let host = self.handle_request_v4().await?;

                let local_addr = self
                    .dispatcher
                    .dispatch(&host)
                    .await
                    .wrap_err_with(dispatch_error)?;

                self.handle_connect_v4(host, local_addr).await
            }
        }
    }

    #[instrument]
    async fn handle_auth(&mut self, handshake: &SocksV5Handshake) -> Result<()> {
        assert_supports_noauth(&handshake)?;

        socksv5::v5::write_auth_method(&mut self.writer, socksv5::v5::SocksV5AuthMethod::Noauth)
            .await?;

        Ok(())
    }

    #[instrument]
    async fn handle_request_v5(&mut self) -> Result<SocketAddr> {
        let request = socksv5::v5::read_request(&mut self.reader).await?;

        match request.command {
            socksv5::v5::SocksV5Command::Connect => {
                let host = match request.host {
                    socksv5::v5::SocksV5Host::Ipv4(ip) => {
                        SocketAddr::new(IpAddr::V4(ip.into()), request.port)
                    }
                    socksv5::v5::SocksV5Host::Ipv6(ip) => {
                        SocketAddr::new(IpAddr::V6(ip.into()), request.port)
                    }
                    socksv5::v5::SocksV5Host::Domain(domain) => {
                        let domain = String::from_utf8(domain)?;
                        let mut addr = match lookup((domain.as_str(), request.port)).await {
                            Ok(addr) => addr,
                            Err(err) => {
                                socksv5::v5::write_request_status(
                                    &mut self.writer,
                                    socksv5::v5::SocksV5RequestStatus::HostUnreachable,
                                    socksv5::v5::SocksV5Host::Ipv4([0, 0, 0, 0]),
                                    0,
                                )
                                .await?;
                                return Err(err.note(lookup_note()).note(safe_to_ignore_note()));
                            }
                        };
                        addr.set_port(request.port);
                        addr
                    }
                };

                Ok(host)
            }
            cmd => {
                socksv5::v5::write_request_status(
                    &mut self.writer,
                    socksv5::v5::SocksV5RequestStatus::CommandNotSupported,
                    socksv5::v5::SocksV5Host::Ipv4([0, 0, 0, 0]),
                    0,
                )
                .await?;
                Err(unsupported_v5_command_error(&cmd))
            }
        }
    }

    #[instrument]
    async fn handle_connect_v5(
        &mut self,
        address: SocketAddr,
        local_addr: IpAddr,
    ) -> Result<TcpStream> {
        let server_socket = try_bind_socket(local_addr)?;

        let server_stream = server_socket.connect(address).await;

        match server_stream {
            Ok(server_stream) => {
                socksv5::v5::write_request_status(
                    &mut self.writer,
                    socksv5::v5::SocksV5RequestStatus::Success,
                    socksv5::v5::SocksV5Host::Ipv4([0, 0, 0, 0]),
                    0,
                )
                .await?;
                Ok(server_stream)
            }
            Err(err) => {
                // Unix error codes.
                // TODO: handle Windows error codes.
                let status = match err.raw_os_error() {
                    // ENETUNREACH
                    Some(101) => socksv5::v5::SocksV5RequestStatus::NetworkUnreachable,
                    // ETIMEDOUT
                    Some(110) => socksv5::v5::SocksV5RequestStatus::TtlExpired,
                    // ECONNREFUSED
                    Some(111) => socksv5::v5::SocksV5RequestStatus::ConnectionRefused,
                    // EHOSTUNREACH
                    Some(113) => socksv5::v5::SocksV5RequestStatus::HostUnreachable,
                    // Unhandled error code
                    _ => socksv5::v5::SocksV5RequestStatus::ServerFailure,
                };
                socksv5::v5::write_request_status(
                    &mut self.writer,
                    status,
                    socksv5::v5::SocksV5Host::Ipv4([0, 0, 0, 0]),
                    0,
                )
                .await?;
                Err(eyre::eyre!(err).wrap_err(connect_error(&address)))
            }
        }
    }

    #[instrument]
    async fn handle_request_v4(&mut self) -> Result<SocketAddr> {
        let request = socksv5::v4::read_request(&mut self.reader).await?;

        match request.command {
            socksv5::v4::SocksV4Command::Connect => Ok(match request.host {
                socksv5::v4::SocksV4Host::Ip(ip) => {
                    SocketAddr::new(IpAddr::V4(ip.into()), request.port)
                }
                socksv5::v4::SocksV4Host::Domain(domain) => {
                    let domain = String::from_utf8(domain)?;
                    let addr = match lookup((domain.as_str(), request.port)).await {
                        Ok(addr) => addr,
                        Err(err) => {
                            socksv5::v4::write_request_status(
                                &mut self.writer,
                                socksv5::v4::SocksV4RequestStatus::Failed,
                                [0, 0, 0, 0],
                                0,
                            )
                            .await?;
                            return Err(err);
                        }
                    };
                    addr
                }
            }),
            cmd => {
                socksv5::v4::write_request_status(
                    &mut self.writer,
                    socksv5::v4::SocksV4RequestStatus::Failed,
                    [0, 0, 0, 0],
                    0,
                )
                .await?;
                Err(unsupported_v4_command_error(&cmd))
            }
        }
    }

    #[instrument]
    async fn handle_connect_v4(
        &mut self,
        address: SocketAddr,
        local_addr: IpAddr,
    ) -> Result<TcpStream> {
        let server_socket = try_bind_socket(local_addr)?;

        let server_stream = server_socket.connect(address).await;

        match server_stream {
            Ok(server_stream) => {
                socksv5::v4::write_request_status(
                    &mut self.writer,
                    socksv5::v4::SocksV4RequestStatus::Granted,
                    [0, 0, 0, 0],
                    0,
                )
                .await?;
                Ok(server_stream)
            }
            Err(err) => {
                socksv5::v4::write_request_status(
                    &mut self.writer,
                    socksv5::v4::SocksV4RequestStatus::Failed,
                    [0, 0, 0, 0],
                    0,
                )
                .await?;
                Err(eyre::eyre!(err).wrap_err(connect_error(&address)))
            }
        }
    }
}

fn connect_error(address: &SocketAddr) -> Report {
    eyre::eyre!(format!("Failed to connect to address `{}`", address))
        .note("This error usually happens when the proxy fails to contact a remote host.")
        .note(safe_to_ignore_note())
}

fn resolve_host_error<T>(host: &T) -> Report
where
    T: Debug,
{
    eyre::eyre!("Failed to resolve the host `{:?}`", *host)
}

fn dispatch_error() -> Report {
    eyre::eyre!("An error occurred during dispatching")
}

fn unsupported_v4_command_error(cmd: &SocksV4Command) -> Report {
    eyre::eyre!("Unsupported SOCKSv4 proxy command `{:?}`", cmd)
}

fn unsupported_v5_command_error(cmd: &SocksV5Command) -> Report {
    eyre::eyre!("Unsupported SOCKSv4 proxy command `{:?}`", cmd)
}

fn unaccessible_local_address_error(addr: &IpAddr) -> Report {
    eyre::eyre!(format!("The local address `{}` is not accessible.", addr)).suggestion(
        "Please ensure that it matches an existing network \
        interface on your computer by inspecting the output of `dispatch list`.",
    )
}

fn http_header_error(out: &str) -> Report {
    let first_http_line = out.split("\r\n").next().unwrap();
    eyre::eyre!(eyre!(
        "The proxy received `{}` ({} additional bytes not shown), which looks like an HTTP \
        request. Please ensure that you have properly configured the proxy as a SOCKS \
        proxy and not an HTTP proxy.",
        first_http_line,
        out.len() - first_http_line.len()
    ))
}

fn unsupported_auth_error() -> Report {
    eyre::eyre!("Only the NOAUTH SOCKS proxy authentication scheme is supported.").suggestion(
        "Please ensure that you haven't provided authentication credentials to your system's \
                proxy configuration.",
    )
}

fn lookup_note() -> &'static str {
    "This error usually happens when an application tries to contact a domain name that does not exist."
}

fn safe_to_ignore_note() -> String {
    use owo_colors::OwoColorize;
    format!("{} {}", "It is safe to ignore in most cases.".bold(), "However, if you notice a degradation in service because of this error, please file an issue.")
}
