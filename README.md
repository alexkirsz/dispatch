# dispatch

A SOCKS proxy that balances traffic between network interfaces.

_Should work on macOS, Windows, and Linux. Only tested on macOS for now._

This is a Rust rewrite of [dispatch-proxy](https://github.com/alexkirsz/dispatch-proxy), originally written in CoffeeScript and targeting Node.js.

## Quick links

- [Installation](#installation)
- [Rationale](#rationale)
- [Use Cases](#use-cases)
- [Usage](#usage)
- [Examples](#examples)
- [How It Works](#how-it-works)
- [License](#license)

## Installation

You'll need Rust version 1.51.0 or later. You can use [rustup](https://rustup.rs/) to install the latest version of the Rust compiler toolchain.

```
cargo install dispatch-proxy
```

## Rationale

You often find yourself with multiple unused internet connections—be it 5G mobile hotspot or a free Wi-Fi network—that your system won't let you use alongside your primary one.

For instance, my first student residence used to provide me with cabled and wireless internet accesses. Both were separately capped at a bandwidth 1,200kB/s. My 3G mobile internet access provided me with an additional 400kB/s. Combining all of these with dispatch and a download manager resulted in a 2,800kB/s effective bandwidth!

## Use cases

The possibilities are endless:

- Use it with a download manager or a BitTorrent client, combining multiple connections' bandwidth when downloading single files;
- Combine as many interfaces as you have access to into a single load-balanced interface;
- Run different apps on separate interfaces with multiple proxies (e.g. for balancing download/upload);
- Create a hotspot proxy at home that connects through Ethernet and your 5G card for all your mobile devices;
- etc.

## Usage

```
$ dispatch

  dispatch 0.1.0
  A proxy that balances traffic between multiple internet connections

  USAGE:
      dispatch [FLAGS] <SUBCOMMAND>

  FLAGS:
      -d, --debug      Write debug logs to stdout instead of a file
      -h, --help       Prints help information
      -V, --version    Prints version information

  SUBCOMMANDS:
      help     Prints this message or the help of the given subcommand(s)
      list     Lists all available network interfaces
      start    Starts the SOCKS proxy server
```

```
$ dispatch start -h

  dispatch-start 0.1.0
  Starts the SOCKS proxy server

  USAGE:
      dispatch start [OPTIONS] <addresses>...

  FLAGS:
      -h, --help       Prints help information
      -V, --version    Prints version information

  OPTIONS:
          --ip <ip>        Which IP to accept connections from [default: 127.0.0.1]
          --port <port>    Which port to listen to for connections [default: 1080]

  ARGS:
      <addresses>...    The network interface IP addresses to dispatch to, in the form of <address>[@priority]
```

## Examples

```
$ dispatch list
```

Lists all available network interfaces.

```
$ dispatch start 10.0.0.0 fdaa:bbcc:ddee:0:1:2:3:4
```

Dispatch incoming connections to local addresses `10.0.0.0` and `fdaa:bbcc:ddee:0:1:2:3:4`.

```
$ dispatch start 10.0.0.0@7 10.0.0.1@3
```

Dispatch incoming connections to `10.0.0.0` 7 times out of 10 and to `10.0.0.1` 3 times out of 10.

## How It Works

Whenever the SOCKS proxy server receives an connection request to an address or domain, it selects one of the provided local addresses using the [Weighted Round Robin](https://en.wikipedia.org/wiki/Weighted_round_robin) algorithm. All further connection traffic will then go through the interface corresponding to the selected local address.

**Beware:** If the requested address or domain resolves to an IPv4 (resp. IPv6) address, an IPv4 (resp. IPv6) local address must be provided.

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
