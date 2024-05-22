use std::net::Ipv4Addr;
use vpn::{SocketFd, TunSocket};

pub fn main() {
    println!("Hello, world!");
    let tun = TunSocket::new("tun1").unwrap();
    tun.set_address(Ipv4Addr::new(10, 0, 0, 1)).unwrap();
    tun.set_netmask(Ipv4Addr::new(255, 255, 255, 0)).unwrap();
    tun.enabled(true).unwrap();

    let socket = SocketFd::new().unwrap();

    let mut buf = [0; 4096];
    loop {
        // packet recieved from tun
        let amount = tun.read(&mut buf).unwrap();
        let slice = Ipv4HeaderSlice::from_slice(&buf[0..amount]).unwrap();
        let source_addr = slice.source_addr();
        println!(
            "Packet from: {} to: {}, total length {}",
            slice.source_addr(),
            slice.destination_addr(),
            slice.total_len()
        );

        let server_ip = source_addr.octets();
        let server_ip = (server_ip[0], server_ip[1], server_ip[2], server_ip[3]);

        if amount > 0 {
            // replace source_addr with our server address and port
            socket.send_to(&buf[0..amount], server_ip, 12345);
        }

        // packet recieved from socket
        let (recv_amount, recv_buf) = socket.recv_from().unwrap();

        if recv_amount > 0 {
            let amount = tun.write(&recv_buf);
            println!("Wrote {} bytes to tun", amount);
        }
    }
}
