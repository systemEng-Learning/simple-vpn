use clap::Parser;
use nix::sys::select::{select, FdSet};
use std::net::Ipv4Addr;
use std::net::Ipv4Addr;
use std::os::unix::io::{AsRawFd, BorrowedFd};
use vpn::{SocketFd, TunSocket};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    client: bool,
    #[arg(short, long)]
    server: bool,
    #[arg(short, long)]
    ip_peer: Option<String>,
    #[arg(short, long)]
    port: Option<u16>,
}

pub fn main() {
    let cli = Cli::parse();

    if cli.server && cli.client {
        eprintln!("Please specify either --client or --server, not both");
        return;
    }

    let ip_peer = cli
        .ip_peer
        .unwrap_or_else(|| "Please specify --ip-peer e.g 172.26.0.3".to_string());
    let port = cli.port.unwrap_or(8080);

    if ip_peer.parse::<Ipv4Addr>().is_err() {
        eprintln!("Invalid IP address");
        return;
    }

    let ip_peer = ip_peer.parse::<Ipv4Addr>().unwrap();

    println!("IP: {}, Port: {}", ip_peer, port);

    if cli.server {
        run_tun(
            "tun-server",
            Ipv4Addr::new(10, 0, 0, 1),
            ip_peer,
            true,
            port,
        );
    } else if cli.client {
        run_tun(
            "tun-client",
            Ipv4Addr::new(10, 0, 0, 2),
            ip_peer,
            false,
            port,
        );
    } else {
        eprintln!("Please specify either --client or --server");
    }
}

fn run_tun(tun_name: &str, local_ip: Ipv4Addr, ip_peer: Ipv4Addr, is_server: bool, port: u16) {
    let tun = TunSocket::new(tun_name).unwrap();
    tun.set_address(local_ip).unwrap();
    tun.set_netmask(Ipv4Addr::new(255, 255, 255, 0)).unwrap();
    tun.enabled(true).unwrap();

    let socket = if is_server {
        SocketFd::new(true).unwrap() // Server binds to a specific port
    } else {
        SocketFd::new(false).unwrap() // Client does not bind to a specific port
    };

    let tun_fd = unsafe { BorrowedFd::borrow_raw(tun.as_raw_fd()) };
    let socket_fd = unsafe { BorrowedFd::borrow_raw(socket.as_raw_fd()) };
    let mut buf = [0; 4096];

    loop {
        let mut read_fds = FdSet::new();
        read_fds.insert(tun_fd);
        read_fds.insert(socket_fd);

        match select(None, &mut read_fds, None, None, None) {
            Ok(_) => {
                if read_fds.contains(tun_fd) {
                    match tun.read(&mut buf) {
                        Ok(amount) => {
                            let server_ip = ip_peer.octets();
                            let server_ip =
                                (server_ip[0], server_ip[1], server_ip[2], server_ip[3]);
                            if amount > 0 {
                                socket.send_to(&buf[0..amount], server_ip, port);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading from TUN: {}", e);
                            break;
                        }
                    }
                }

                if read_fds.contains(socket_fd) {
                    match socket.recv_from() {
                        Ok((recv_amount, recv_buf)) => {
                            if recv_amount > 0 {
                                //print out something here
                                println!("Received {} bytes from socket", recv_amount);
                                let amount = tun.write(&recv_buf);
                                println!("Wrote {} bytes to tun", amount);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading from socket: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("select error: {}", e);
                break;
            }
        }
    }
}
