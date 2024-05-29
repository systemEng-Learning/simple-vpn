use std::env;
use std::os::unix::io::AsRawFd;

use tunnel::net::Net;
use tunnel::select::{select, FdSet};
use tunnel::tun::TunSocket;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("You didn't supply enough arguments");
    }
    let (name, remote_addr, is_client, port) = parse_args(args);
    if is_client && remote_addr == "" {
        panic!("You must supply a server ip and port number for a client");
    }

    let net = Net::new(&remote_addr, port, is_client).unwrap();
    let tunnel = TunSocket::new(&name).unwrap();
    run(net, tunnel);
}

fn parse_args(args: Vec<String>) -> (String, String, bool, u16) {
    let mut name = String::from("playtun");
    let mut remote_addr = String::from("");
    let mut is_client = false;
    let mut port = 2000;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--client" || args[i] == "--c" {
            is_client = true;
            i += 1;
            continue;
        }

        if (args[i] == "--name" || args[i] == "-n") && i+1 < args.len() {
            name = args[i+1].clone();
        }

        

        if (args[i] == "--address" || args[i] == "-a") && i+1 < args.len() {
            remote_addr = args[i+1].clone();
        }

        if (args[i] == "--port" || args[i] == "-p") && i+1 < args.len() {
            port = args[i+1].parse().unwrap();
        }
        i += 2;
    }
    return (name, remote_addr, is_client, port)
}

fn run(mut net: Net, tunnel: TunSocket) {
    let mut tun2net = 0;
    let mut net2tun = 0;
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
                    let (buf, amt) = net.recv().unwrap();
                    println!("NET2TUN {net2tun}: Read {amt} from network");
                    let amt = tunnel.write(buf.as_slice());
                    println!("NET2TUN {net2tun}: Written {amt} to tunnel");
                }

                if fdset.is_set(tun_fd) {
                    tun2net += 1;
                    let amt = tunnel.read(&mut dst).unwrap();
                    println!("TUN2NET {tun2net}: Read {amt} from tunnel");
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
