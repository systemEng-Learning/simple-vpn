use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::{
    io,
    os::fd::{AsRawFd, RawFd},
};

use etherparse::{Ipv4HeaderSlice, checksum};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::tunerror;

const IPV4_HEADER_LEN: usize = 20;

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

    pub fn send(&self, buf: &mut [u8], size: usize) -> usize {
        let version = buf[0] >> 4;
        if version != 4 {
            return 0;
        }
        let new_size = Self::encrypt(buf, size);
        let buf = &buf[..new_size];
        if self.ip_map.is_none() {
            let _ = self.socket.send(buf).unwrap();
        } else {
            let slice = Ipv4HeaderSlice::from_slice(&buf);
            if slice.is_err() {
                println!("{:?}", slice.err().unwrap());
                return 0;
            }
            let destination_ip = slice.unwrap().destination_addr();
            let client_ip = self.ip_map.as_ref().unwrap().get(&destination_ip);
            if client_ip.is_some() {
                let _ = self.socket.send_to(buf, client_ip.unwrap()).unwrap();
            }
        }
        new_size
    }

    fn encrypt(buf: &mut [u8], size: usize) -> usize {
        let mut length = u16::from_be_bytes([buf[2], buf[3]]);
        println!("Original size: {size}, Stated size: {length}");
        buf[size] = 5;
        length += 1;
        let bytes = length.to_be_bytes();
        buf[2] = bytes[0];
        buf[3] = bytes[1];
        println!("Decoded size: {}", u16::from_be_bytes(bytes));
        let mut header_length = (buf[0] & 15) as usize;
        header_length *= 4;
        println!("Header Length {header_length}");
        Self::set_header_checksum(&mut buf[..header_length]);
        println!("{:?}", &mut buf[..header_length]);
        size + 1
    }

    pub fn recv(&mut self) -> Result<(Vec<u8>, usize), tunerror::Error> {
        let mut buf = [0; 4096];
        let recv_buf = unsafe { &mut *(&mut buf[..] as *mut [u8] as *mut [MaybeUninit<u8>]) };
        let (amount, remote_sock) = self.socket.recv_from(recv_buf).unwrap();
        let version = buf[0] >> 4;
        if version != 4 {
            return Err(tunerror::Error::Message("Invalid packet".to_owned()));
        }
        let new_size = Self::decrypt(&mut buf, amount);
        if self.ip_map.is_some() {
            let slice = Ipv4HeaderSlice::from_slice(&buf[..new_size]);
            match slice {
                Ok(header) => {
                    let source_ip = header.source_addr();
                    self.ip_map.as_mut().unwrap().insert(source_ip, remote_sock);
                },
                Err(e) => { 
                    println!("{:?}", e); 
                    return Err(tunerror::Error::Message("Invalid packet".to_owned()))
                },
            }
        }
        let buf_vec = buf[..new_size].to_vec();
        Ok((buf_vec, amount))
    }

    fn decrypt(buf: &mut [u8], size: usize) -> usize {
        let mut length = u16::from_be_bytes([buf[2], buf[3]]);
        length -= 1;
        let bytes = length.to_be_bytes();
        buf[2] = bytes[0];
        buf[3] = bytes[1];
        let mut header_length = (buf[0] & 15) as usize;
        header_length *= 4;
        Self::set_header_checksum(&mut buf[..header_length]);
        size - 1
    }

    fn set_header_checksum(buf: &mut [u8]) {
        let mut csum = checksum::Sum16BitWords::new();
        for x in (0..10).step_by(2) {
            csum = csum.add_2bytes([buf[x], buf[x+1]]);
        }
        csum = csum.add_4bytes([buf[12], buf[13], buf[14], buf[15]]);
        csum = csum.add_4bytes([buf[16], buf[17], buf[18], buf[19]]);
        if buf.len() > IPV4_HEADER_LEN {
            println!("here");
            csum = csum.add_slice(&buf[IPV4_HEADER_LEN..]);
        }
        let sum = csum.ones_complement().to_be_bytes();
        buf[10] = sum[0];
        buf[11] = sum[1];
    }
}
