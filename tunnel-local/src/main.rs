use std::env;
use std::os::unix::io::AsRawFd;
use std::process::Command;

use tunnel::net::Net;
use tunnel::packet;
use tunnel::select::{select, FdSet};
use tunnel::tun::TunSocket;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let (name, remote_addr, local_ip, key, is_client, port, host_port) = parse_args(args);
    if local_ip == "" {
        panic!("You must supply a tun dev ip address");
    }
    if is_client && remote_addr == "" {
        panic!("You must supply a server ip and port number for a client");
    }
    if key.len() > 32 {
        panic!("Password length must be less than or equal to 32");
    }

    let mut net = Net::new(&remote_addr, port, is_client, key).unwrap();
    let tunnel = TunSocket::new(&name).unwrap();
    setup_link_dev(&name, &local_ip, is_client);
    let local_ip = parse_ip(local_ip);
    if is_client {
        client_handshake(&mut net, &local_ip)
    }
    run(net, tunnel, is_client, &local_ip, host_port);
}

fn parse_args(args: Vec<String>) -> (String, String, String, String, bool, u16, u16) {
    let mut name = String::from("playtun");
    let mut remote_addr = String::from("");
    let mut key = String::from("");
    let mut local_ip = String::from("");
    let mut is_client = false;
    let mut port = 2000;
    let mut host_port = 8080;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--client" || args[i] == "--c" {
            is_client = true;
            i += 1;
            continue;
        }

        if (args[i] == "--name" || args[i] == "-n") && i + 1 < args.len() {
            name = args[i + 1].clone();
        }

        if (args[i] == "--address" || args[i] == "-a") && i + 1 < args.len() {
            remote_addr = args[i + 1].clone();
        }

        if (args[i] == "--port" || args[i] == "-p") && i + 1 < args.len() {
            port = args[i + 1].parse().unwrap();
        }

        if (args[i] == "--key" || args[i] == "-k") && i + 1 < args.len() {
            key = args[i + 1].clone();
        }

        if (args[i] == "--local" || args[i] == "-l") && i + 1 < args.len() {
            local_ip = args[i + 1].clone();
        }

        if (args[i] == "--site-port" || args[i] == "-s") && i + 1 < args.len() {
            host_port = args[i + 1].parse().unwrap();
        }

        i += 2;
    }
    return (name, remote_addr, local_ip, key, is_client, port, host_port);
}

fn setup_link_dev(name: &str, ip_addr: &str, is_client: bool) {
    let mut command = format!("ip link set dev {name} up; ip addr add {ip_addr}/24 dev {name}");
    if is_client {
        command = format!("{command}; sysctl -w net.ipv4.conf.{name}.route_localnet=1");
    }
    let _ = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .expect("Failed to execute process");

}

fn parse_ip(ip: String) -> Vec<u8> {
    let mut iter = ip.split('.');
    let mut ip = vec![];
    while let Some(c) = iter.next() {
        ip.push(c.parse::<u8>().unwrap());
    }
    ip
}

fn client_handshake(net: &mut Net, ip: &[u8]) {
    let hello_packet = packet::create_handshake_packet(&ip[..4].try_into().unwrap());
    let mut dst: [u8; 4096] = [0; 4096];
    for i in 0..hello_packet.len() {
        dst[i] = hello_packet[i];
    }
    let amt = net.send(&mut dst, hello_packet.len());
    println!("HANDSHAKE: Written {amt} to network");
}

fn run(mut net: Net, tunnel: TunSocket, is_client: bool, local_ip: &[u8], host_port: u16) {
    let mut tun2net = 0;
    let mut net2tun = 0;
    let mut port = 0;
    loop {
        let mut fdset = FdSet::new();
        let net_fd = net.as_raw_fd();
        let tun_fd = tunnel.as_raw_fd();
        fdset.set(net_fd);
        fdset.set(tun_fd);
        let max_fd = net_fd.max(tun_fd);
        let mut dst: [u8; 4096] = [0; 4096];
        match select(max_fd + 1, Some(&mut fdset), None, None, None) {
            Ok(res) => {
                println!("select result: {res}");
                if fdset.is_set(net_fd) {
                    net2tun += 1;
                    let (mut buf, amt) = net.recv().unwrap();
                    println!("NET2TUN {net2tun}: Read {amt} from network");
                    if is_client && packet::get_version(&buf) == 4 {
                        port = packet::change_address_and_port(
                            &mut buf,
                            &[127, 0, 0, 1],
                            host_port,
                            false,
                        );
                    }
                    if is_client || !packet::is_handshake_packet(buf.as_slice()) {
                        let amt = tunnel.write(buf.as_slice());
                        println!("NET2TUN {net2tun}: Written {amt} to tunnel");
                    }
                }

                if fdset.is_set(tun_fd) {
                    tun2net += 1;
                    let amt = tunnel.read(&mut dst).unwrap();
                    println!("TUN2NET {tun2net}: Read {amt} from tunnel");
                    if is_client && packet::get_version(&dst) == 4 {
                        let _ = packet::change_address_and_port(&mut dst, local_ip, port, true);
                    }
                    let amt = net.send(&mut dst, amt);
                    println!("TUN2NET {tun2net}: Written {amt} to network");
                }
            }
            Err(err) => {
                println!("Failed to select {:?}", err);
            }
        }
    }
}
