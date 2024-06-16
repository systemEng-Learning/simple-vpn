use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::vec;
use std::{
    io,
    os::fd::{AsRawFd, RawFd},
};

use etherparse::{Ipv4HeaderSlice, Ipv6HeaderSlice};
use ring::aead::Aad;
use ring::aead::BoundKey;
use ring::aead::Nonce;
use ring::aead::NonceSequence;
use ring::aead::OpeningKey;
use ring::aead::SealingKey;
use ring::aead::UnboundKey;
use ring::aead::AES_256_GCM;
use ring::aead::NONCE_LEN;
use ring::error::Unspecified;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::packet;
use crate::tunerror;
const IPV6_HEADER_LEN: usize = 40;

struct CounterNonceSequence(u32);

impl NonceSequence for CounterNonceSequence {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        let mut nonce_bytes = vec![0; NONCE_LEN];

        let bytes = self.0.to_be_bytes();
        nonce_bytes[8..].copy_from_slice(&bytes);
        self.0 += 1;
        Nonce::try_assume_unique_for_key(&nonce_bytes)
    }
}

pub struct Net {
    fd: RawFd,
    pub socket: Socket,
    ip_map: Option<HashMap<IpAddr, SockAddr>>,
    key: Vec<u8>,
}

impl AsRawFd for Net {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Net {
    pub fn new(
        remote_addr: &str,
        port: u16,
        is_client: bool,
        key: String,
    ) -> Result<Net, io::Error> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        let net: Net;
        socket.set_reuse_address(true)?;
        let mut key_bytes;
        if key.len() > 0 {
            key_bytes = vec![0; AES_256_GCM.key_len()];
            for (i, b) in key.bytes().enumerate() {
                key_bytes[i] = b;
            }
        } else {
            key_bytes = vec![];
        }

        if is_client {
            let address: SocketAddr = remote_addr.parse().unwrap();
            let address = address.into();
            socket.connect(&address)?;
            net = Net {
                fd: socket.as_raw_fd(),
                socket: socket,
                ip_map: None,
                key: key_bytes,
            };
        } else {
            let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into();
            socket.bind(&bind_addr)?;
            let map: HashMap<IpAddr, SockAddr> = HashMap::new();
            net = Net {
                fd: socket.as_raw_fd(),
                socket: socket,
                ip_map: Some(map),
                key: key_bytes,
            };
        }
        Ok(net)
    }

    /// Sends an IP packet to a UDP endpoint.
    pub fn send(&self, buf: &mut [u8], size: usize) -> usize {
        let version = buf[0] >> 4;
        if version != 4 && version != 6 {
            return 0;
        }
        let mut new_size = size;
        if self.key.len() > 0 {
            new_size = self
                .encrypt(buf, size, version)
                .expect("Encryption process had an error");
        }
        let buf = &buf[..new_size];
        if self.ip_map.is_none() {
            let _ = self.socket.send(buf).unwrap();
        } else {
            let destination_ip;
            if version == 4 {
                let slice = Ipv4HeaderSlice::from_slice(&buf);
                if slice.is_err() {
                    println!("{:?}", slice.err().unwrap());
                    return 0;
                }
                destination_ip = IpAddr::V4(slice.unwrap().destination_addr());
            } else {
                let slice = Ipv6HeaderSlice::from_slice(&buf);
                if slice.is_err() {
                    println!("{:?}", slice.err().unwrap());
                    return 0;
                }
                destination_ip = IpAddr::V6(slice.unwrap().destination_addr());
            }
            let client_ip = self.ip_map.as_ref().unwrap().get(&destination_ip);
            if client_ip.is_some() {
                let _ = self.socket.send_to(buf, client_ip.unwrap()).unwrap();
            }
        }
        new_size
    }

    /// Encrypts a packet to be sent over the network
    fn encrypt(&self, buf: &mut [u8], size: usize, version: u8) -> Result<usize, Unspecified> {
        let header_length = self.configure_header(buf, version, true);
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)?;
        let nonce_sequence = CounterNonceSequence(1);
        let mut sealing_key = SealingKey::new(unbound_key, nonce_sequence);
        let associated_data = Aad::empty();

        let tag = sealing_key
            .seal_in_place_separate_tag(associated_data, &mut buf[header_length..size])?;

        // Add the tag data to the buffer
        for i in 0..AES_256_GCM.tag_len() {
            buf[size + i] = tag.as_ref()[i];
        }
        Ok(size + AES_256_GCM.tag_len())
    }

    /// Receives a packet from the other peer and decrypts it. Only IPv4 packets can be processed
    /// TODO: IPv6
    pub fn recv(&mut self) -> Result<(Vec<u8>, usize), tunerror::Error> {
        let mut buf = [0; 4096];
        let recv_buf = unsafe { &mut *(&mut buf[..] as *mut [u8] as *mut [MaybeUninit<u8>]) };
        let (amount, remote_sock) = self.socket.recv_from(recv_buf).unwrap();
        let version = buf[0] >> 4;
        if version != 4 && version != 6 {
            return Err(tunerror::Error::Message("Invalid packet".to_owned()));
        }
        let mut new_size = amount;
        if self.key.len() > 0 {
            new_size = self
                .decrypt(&mut buf, amount, version)
                .expect("Decryption process had an error");
        }
        if self.ip_map.is_some() {
            if version == 4 {
                let slice = Ipv4HeaderSlice::from_slice(&buf[..new_size]);
                match slice {
                    Ok(header) => {
                        let source_ip = header.source_addr();
                        self.ip_map
                            .as_mut()
                            .unwrap()
                            .insert(IpAddr::V4(source_ip), remote_sock);
                    }
                    Err(e) => {
                        println!("{:?}", e);
                        return Err(tunerror::Error::Message("Invalid packet".to_owned()));
                    }
                }
            } else {
                let slice = Ipv6HeaderSlice::from_slice(&buf[..new_size]);
                match slice {
                    Ok(header) => {
                        let source_ip = header.source_addr();
                        self.ip_map
                            .as_mut()
                            .unwrap()
                            .insert(IpAddr::V6(source_ip), remote_sock);
                    }
                    Err(e) => {
                        println!("{:?}", e);
                        return Err(tunerror::Error::Message("Invalid packet".to_owned()));
                    }
                }
            }
        }
        let buf_vec = buf[..new_size].to_vec();
        Ok((buf_vec, amount))
    }

    /// Decrypts a packet from the network using AES
    fn decrypt(&self, buf: &mut [u8], size: usize, version: u8) -> Result<usize, Unspecified> {
        let header_length = self.configure_header(buf, version, false);
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)?;
        let nonce_sequence = CounterNonceSequence(1);
        let mut opening_key = OpeningKey::new(unbound_key, nonce_sequence);
        let associated_data = Aad::empty();
        let _ = opening_key.open_in_place(associated_data, &mut buf[header_length..size])?;
        Ok(size - AES_256_GCM.tag_len())
    }

    /// Sets a new length; the length increases if it's an encryption process, else it decreases.
    /// The IPv4 header format https://en.wikipedia.org/wiki/IPv4#Header helps us know where
    /// the needed data is stored for ipv4 packets. The IPv4 header format
    /// https://en.wikipedia.org/wiki/IPv6_packet#Fixed_header helps us know where. Returns the header length
    fn configure_header(&self, buf: &mut [u8], version: u8, is_encrypt: bool) -> usize {
        let mut length;
        if version == 4 {
            length = u16::from_be_bytes([buf[2], buf[3]]);
        } else {
            length = u16::from_be_bytes([buf[4], buf[5]]);
        }
        if is_encrypt {
            length += AES_256_GCM.tag_len() as u16;
        } else {
            length -= AES_256_GCM.tag_len() as u16;
        }
        let bytes = length.to_be_bytes();
        let mut header_length;
        if version == 4 {
            buf[2] = bytes[0];
            buf[3] = bytes[1];
            header_length = (buf[0] & 15) as usize;
            header_length *= 4;
            packet::set_header_checksum(&mut buf[..header_length]);
        } else {
            buf[4] = bytes[0];
            buf[5] = bytes[1];
            header_length = IPV6_HEADER_LEN;
        }
        header_length
    }
}
