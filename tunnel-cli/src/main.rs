use std::os::unix::io::AsRawFd;

use tunnel::net::Net;
use tunnel::select::{select, FdSet};
use tunnel::tun::TunSocket;

pub fn main() {
    println!("Hello, world!");
    let mut net = Net::new("", 2000, false).unwrap();
    let tunnel = TunSocket::new("playtun").unwrap();
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
                    let buf = net.recv().unwrap();
                    println!("NET2TUN {net2tun}: Read {} from network", buf.len());
                    let amt = tunnel.write(buf.as_slice());
                    println!("NET2TUN {net2tun}: Written {amt} to tunnel");
                }

                if fdset.is_set(tun_fd) {
                    tun2net += 1;
                    let amt = tunnel.read(&mut dst).unwrap();
                    println!("TUN2NET {tun2net}: Read {amt} from tunnel");
                    net.send(&dst[..amt]);
                    println!("TUN2NET {tun2net}: Written {amt} to network");
                }
            }
            Err(err) => {
                println!("Failed to select {:?}", err);
            }
        }
    }
}
