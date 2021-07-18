use crate::net::bind_socket;
use owo_colors::OwoColorize;
use pnet_datalink::interfaces;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

pub fn list() {
    let mut table = Table::new();
    table.max_column_width = 40;
    table.style = TableStyle::extended();

    for interface in interfaces()
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .filter(|interface| interface.ips.len() > 0)
    {
        let ips = {
            let mut ips: Vec<_> = interface
                .ips
                .into_iter()
                .map(|ip| ip.ip())
                .filter(|ip| bind_socket(*ip).is_ok())
                .collect();
            ips.sort_by_key(|ip| ip.is_ipv6());
            ips
        };

        if ips.is_empty() {
            continue;
        }

        table.add_row(Row::new(vec![
            TableCell::new_with_alignment(interface.name.bold(), 1, Alignment::Right),
            TableCell::new_with_alignment(
                ips.iter()
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
