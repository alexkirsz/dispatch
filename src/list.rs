use std::net::IpAddr;

use crate::net::bind_socket;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use owo_colors::OwoColorize;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

pub fn list() {
    let mut table = Table::new();
    table.max_column_width = 41;
    table.style = TableStyle::extended();

    for interface in NetworkInterface::show()
        .expect("failed to retrieve network interfaces")
        .into_iter()
        .filter(|interface| !interface.addr.is_empty())
    {
        let addrs = {
            let mut addrs: Vec<_> = interface
                .addr
                .into_iter()
                .map(|addr| addr.ip())
                .filter(|addr| !is_local_address(addr))
                .filter(|addr| bind_socket(*addr, "".to_string()).is_ok())
                .collect();
            addrs.sort_by_key(|addr| addr.is_ipv6());
            addrs
        };

        if addrs.is_empty() {
            continue;
        }

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment(interface.name.bold(), 1, Alignment::Right),
            TableCell::new_with_alignment(
                addrs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
                1,
                Alignment::Left,
            ),
        ]));
    }

    println!("{}", table.render());
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
