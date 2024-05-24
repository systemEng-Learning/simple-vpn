use std::collections::HashMap;
use std::fmt::Error;
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::{
    io,
    os::fd::{AsRawFd, RawFd},
};

use etherparse::Ipv4HeaderSlice;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

pub struct Net {
    fd: RawFd,
    pub socket: Socket,
    ip_map: Option<HashMap<Ipv4Addr, SockAddr>>,
}

impl AsRawFd for Net {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Net {
    pub fn new(remote_addr: &str, port: u16, is_client: bool) -> Result<Net, io::Error> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        let net: Net;
        socket.set_reuse_address(true)?;
        if is_client {
            let address: SocketAddr = remote_addr.parse().unwrap();
            let address = address.into();
            socket.connect(&address)?;
            net = Net {
                fd: socket.as_raw_fd(),
                socket: socket,
                ip_map: None,
            };
        } else {
            let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into();
            socket.bind(&bind_addr)?;
            let map: HashMap<Ipv4Addr, SockAddr> = HashMap::new();
            net = Net {
                fd: socket.as_raw_fd(),
                socket: socket,
                ip_map: Some(map),
            };
        }
        Ok(net)
    }

    pub fn send(&self, buf: &[u8]) {
        if self.ip_map.is_none() {
            let _ = self.socket.send(buf).unwrap();
        } else {
            let slice = Ipv4HeaderSlice::from_slice(&buf).unwrap();
            let destination_ip = slice.destination_addr();
            let client_ip = self.ip_map.as_ref().unwrap().get(&destination_ip);
            if client_ip.is_some() {
                let _ = self.socket.send_to(buf, client_ip.unwrap()).unwrap();
            }
        }
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf = [0; 4096];
        let recv_buf = unsafe { &mut *(&mut buf[..] as *mut [u8] as *mut [MaybeUninit<u8>]) };
        let (amount, remote_sock) = self.socket.recv_from(recv_buf).unwrap();
        if self.ip_map.is_some() {
            let slice = Ipv4HeaderSlice::from_slice(&buf[..amount]).unwrap();
            let source_ip = slice.source_addr();
            self.ip_map.as_mut().unwrap().insert(source_ip, remote_sock);
        }
        let buf_vec = buf[..amount].to_vec();
        Ok(buf_vec)
    }
}
