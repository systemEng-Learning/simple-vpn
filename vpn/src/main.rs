use std::net::Ipv4Addr;
use vpn::TunSocket;

pub fn main() {
    println!("Hello, world!");
    let tun = TunSocket::new("tun1").unwrap();
    tun.set_address(Ipv4Addr::new(10, 0, 0, 1)).unwrap();
    tun.set_netmask(Ipv4Addr::new(255, 255, 255, 0)).unwrap();
    tun.enabled(true).unwrap();

    let mut buf = [0; 4096];
    loop {
        let amount = tun.read(&mut buf).unwrap();
        println!("{:?}", &buf[0..amount]);
    }
}
