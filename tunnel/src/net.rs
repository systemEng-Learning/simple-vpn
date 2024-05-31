use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::vec;
use std::{
    io,
    os::fd::{AsRawFd, RawFd},
};

use etherparse::{checksum, Ipv4HeaderSlice};
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

use crate::tunerror;

const IPV4_HEADER_LEN: usize = 20;

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
    ip_map: Option<HashMap<Ipv4Addr, SockAddr>>,
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
            let map: HashMap<Ipv4Addr, SockAddr> = HashMap::new();
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
        if version != 4 {
            return 0;
        }
        let mut new_size = size;
        if self.key.len() > 0 {
            new_size = self
                .encrypt(buf, size)
                .expect("Encryption process had an error");
        }
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

    /// Encrypts a packet to be sent over the network
    fn encrypt(&self, buf: &mut [u8], size: usize) -> Result<usize, Unspecified> {
        let header_length = self.configure_header(buf, true);
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
        if version != 4 {
            return Err(tunerror::Error::Message("Invalid packet".to_owned()));
        }
        let mut new_size = amount;
        if self.key.len() > 0 {
            new_size = self
                .decrypt(&mut buf, amount)
                .expect("Decryption process had an error");
        }
        if self.ip_map.is_some() {
            let slice = Ipv4HeaderSlice::from_slice(&buf[..new_size]);
            match slice {
                Ok(header) => {
                    let source_ip = header.source_addr();
                    self.ip_map.as_mut().unwrap().insert(source_ip, remote_sock);
                }
                Err(e) => {
                    println!("{:?}", e);
                    return Err(tunerror::Error::Message("Invalid packet".to_owned()));
                }
            }
        }
        let buf_vec = buf[..new_size].to_vec();
        Ok((buf_vec, amount))
    }

    /// Decrypts a packet from the network using AES
    fn decrypt(&self, buf: &mut [u8], size: usize) -> Result<usize, Unspecified> {
        let header_length = self.configure_header(buf, false);
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.key)?;
        let nonce_sequence = CounterNonceSequence(1);
        let mut opening_key = OpeningKey::new(unbound_key, nonce_sequence);
        let associated_data = Aad::empty();
        let _ = opening_key.open_in_place(associated_data, &mut buf[header_length..size])?;
        Ok(size - AES_256_GCM.tag_len())
    }

    /// Sets a new length; the length increases if it's an encryption process, else it decreases.
    /// The IPv4 header format https://en.wikipedia.org/wiki/IPv4#Header helps us know where
    /// the needed data is stored. Returns the header length
    fn configure_header(&self, buf: &mut [u8], is_encrypt: bool) -> usize {
        let mut length = u16::from_be_bytes([buf[2], buf[3]]);
        if is_encrypt {
            length += AES_256_GCM.tag_len() as u16;
        } else {
            length -= AES_256_GCM.tag_len() as u16;
        }
        let bytes = length.to_be_bytes();
        buf[2] = bytes[0];
        buf[3] = bytes[1];
        let mut header_length = (buf[0] & 15) as usize;
        header_length *= 4;
        self.set_header_checksum(&mut buf[..header_length]);
        header_length
    }

    /// Due to the length change done by the encryption/decryption process, a new header checksum has
    /// to be calculated. This prevents the kernel from dropping our encrypted/decrypted packets.
    /// This also sets the checksum in the packet bytes indexes.
    fn set_header_checksum(&self, buf: &mut [u8]) {
        let mut csum = checksum::Sum16BitWords::new();
        for x in (0..10).step_by(2) {
            csum = csum.add_2bytes([buf[x], buf[x + 1]]);
        }
        csum = csum.add_4bytes([buf[12], buf[13], buf[14], buf[15]]);
        csum = csum.add_4bytes([buf[16], buf[17], buf[18], buf[19]]);
        if buf.len() > IPV4_HEADER_LEN {
            csum = csum.add_slice(&buf[IPV4_HEADER_LEN..]);
        }
        let sum = csum.ones_complement().to_be().to_be_bytes();
        buf[10] = sum[0];
        buf[11] = sum[1];
    }
}
