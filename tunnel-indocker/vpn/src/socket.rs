use crate::tunerror::Error;
use etherparse::{Ipv4HeaderSlice, Ipv6HeaderSlice};
use libc::close;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::io;
use std::mem::MaybeUninit;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};

#[derive(Default, Debug)]
pub struct SocketFd {
    fd: RawFd,
    socket: Option<Socket>,
}

impl AsRawFd for SocketFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl IntoRawFd for SocketFd {
    fn into_raw_fd(self) -> RawFd {
        self.fd
    }
}

impl Drop for SocketFd {
    fn drop(&mut self) {
        unsafe {
            close(self.fd);
        }
    }
}

impl SocketFd {
    pub fn new(port: u16) -> Result<SocketFd, Error> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();

        let socket_addr = SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), port);

        socket.set_reuse_address(true)?;
        socket.bind(&socket_addr.into())?;

        socket.set_nonblocking(true)?;
        Ok(SocketFd {
            fd: socket.as_raw_fd(),
            socket: Some(socket),
        })
    }

    pub fn send_to(&self, buf: &[u8], ip: (u8, u8, u8, u8), port: u16) {
        let ipv4 = Ipv4Addr::new(ip.0, ip.1, ip.2, ip.3);
        let server_address = SockAddr::from(SocketAddr::new(std::net::IpAddr::V4(ipv4), port));

        let amout = self
            .socket
            .as_ref()
            .unwrap()
            .send_to(&buf, &server_address.into())
            .unwrap();

        println!("Sent {} bytes to socket", amout);
    }

    pub fn recv_from(&self) -> Result<(usize, Vec<u8>), Error> {
        let mut buf = [0; 4096];
        // following line copied from boringtun to resolve the issue with .recv_from()
        // Safety: the `recv_from` implementation promises not to write uninitialised
        // bytes to the buffer, so this casting is safe.
        let src_buf = unsafe { &mut *(&mut buf[..] as *mut [u8] as *mut [MaybeUninit<u8>]) };
        let (amount, recvraddr) = self.socket.as_ref().unwrap().recv_from(src_buf)?;
        let ip_version = buf[0] >> 4;

        println!("Received receieved from {:?}", recvraddr);

        let new_payload = match ip_version {
            4 => {
                let slice = Ipv4HeaderSlice::from_slice(&buf[..amount]).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "failed to parse IPv4 header")
                })?;
                let header_len = slice.ihl() as usize * 4;
                &buf
            }
            6 => {
                let slice = Ipv6HeaderSlice::from_slice(&buf[0..amount]).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "failed to parse IPv6 header")
                })?;
                let header_len = slice.payload_length() as usize;
                &buf
            }
            _ => {
                return Err(Error::IfaceRead(io::Error::last_os_error()));
            }
        };

        Ok((amount, new_payload.to_vec()))
    }
}
