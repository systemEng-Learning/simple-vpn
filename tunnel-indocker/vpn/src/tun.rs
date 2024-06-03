use libc::{
    c_int, c_short, c_uchar, close, ifreq, in_addr, ioctl, open, read, sa_family_t, sockaddr,
    sockaddr_in, socket, write, AF_INET, IFF_RUNNING, IFF_UP, IFNAMSIZ, O_RDWR, SIOCSIFADDR,
    SIOCSIFDSTADDR, SIOCSIFFLAGS, SIOCSIFNETMASK, SOCK_DGRAM,
};

const TUNSETIFF: u64 = 0x400454ca;
const IFF_TUN: c_short = 0x0001;
const IFF_NO_PI: c_short = 0x1000;
const IFF_MULTI_QUEUE: c_short = 0x0100;
use crate::tunerror::Error;
use std::ffi::CString;
use std::io;
use std::net::Ipv4Addr;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::{mem, ptr};

#[derive(Default, Debug)]
pub struct TunSocket {
    fd: RawFd,
    name: String,
    socket_tun: RawFd,
}

impl Drop for TunSocket {
    fn drop(&mut self) {
        unsafe {
            close(self.fd);
            close(self.socket_tun);
        };
    }
}

impl AsRawFd for TunSocket {
    //A trait to extract the raw file descriptor from an underlying object.
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl IntoRawFd for TunSocket {
    //A trait to consume the object and return the raw file descriptor.
    fn into_raw_fd(self) -> RawFd {
        self.fd
    }
}

impl TunSocket {
    pub fn new(dev: &str) -> Result<TunSocket, Error> {
        let dev_name = CString::new(dev).unwrap();
        let dev_name_bytes = dev_name.as_bytes_with_nul();

        if dev_name_bytes.len() >= IFNAMSIZ {
            return Err(Error::InvalidTunnelName);
        }

        let fd = match unsafe { open(b"/dev/net/tun\0".as_ptr() as *const _, O_RDWR) } {
            -1 => return Err(Error::Socket(io::Error::last_os_error())),
            fd => fd,
        };

        let mut ifr: ifreq = unsafe { mem::zeroed() };

        unsafe {
            ptr::copy_nonoverlapping(
                dev_name_bytes.as_ptr() as *mut i8,
                ifr.ifr_name.as_mut_ptr(),
                dev_name_bytes.len(),
            );
        }

        ifr.ifr_ifru.ifru_flags = IFF_TUN | IFF_NO_PI | IFF_MULTI_QUEUE;

        if unsafe { ioctl(fd, TUNSETIFF as _, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }

        let name = dev.to_string();
        // set socket for configuration
        let socket_tun = match unsafe { socket(AF_INET, SOCK_DGRAM, 0) } {
            -1 => return Err(Error::Socket(io::Error::last_os_error())),
            fd => fd,
        };

        Ok(TunSocket {
            fd,
            name,
            socket_tun,
        })
    }

    pub fn read(&self, dst: &mut [u8]) -> Result<usize, Error> {
        match unsafe { read(self.fd, dst.as_mut_ptr() as _, dst.len()) } {
            -1 => Err(Error::IfaceRead(io::Error::last_os_error())),
            n => Ok(n as usize),
        }
    }

    pub fn write(&self, buf: &[u8]) -> usize {
        match unsafe { write(self.fd, buf.as_ptr() as _, buf.len() as _) } {
            -1 => 0,
            n => n as usize,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_get_ifreq(&self) -> ifreq {
        let mut ifr: ifreq = unsafe { mem::zeroed() };
        unsafe {
            ptr::copy_nonoverlapping(
                self.name.as_bytes().as_ptr(),
                ifr.ifr_name.as_mut_ptr() as *mut u8,
                self.name.len(),
            );
        }
        ifr
    }

    fn to_sockaddr(&self, ip: Ipv4Addr) -> sockaddr {
        let ip = ip.octets();
        let mut addr = unsafe { mem::zeroed::<sockaddr_in>() };
        addr.sin_family = AF_INET as sa_family_t;
        addr.sin_port = 0;
        addr.sin_addr = in_addr {
            s_addr: u32::from_ne_bytes(ip),
        };

        let sockaddr: sockaddr = unsafe { mem::transmute(addr) };

        sockaddr
    }

    fn from_sockaddr(&self, sockaddr: sockaddr_in) -> Ipv4Addr {
        let ip = sockaddr.sin_addr.s_addr;
        let [a, b, c, d] = ip.to_ne_bytes();
        Ipv4Addr::new(a, b, c, d)
    }

    pub fn set_address(&self, address: Ipv4Addr) -> Result<(), Error> {
        let mut ifr = self.set_get_ifreq();
        ifr.ifr_ifru.ifru_addr = self.to_sockaddr(address).into();

        if unsafe { ioctl(self.socket_tun, SIOCSIFADDR as _, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }

        Ok(())
    }

    pub fn set_destination(&self, destination: Ipv4Addr) -> Result<(), Error> {
        let mut ifr = self.set_get_ifreq();
        ifr.ifr_ifru.ifru_dstaddr = self.to_sockaddr(destination).into();
        if unsafe { ioctl(self.socket_tun, SIOCSIFDSTADDR, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn set_netmask(&self, netmask: Ipv4Addr) -> Result<(), Error> {
        let mut ifr = self.set_get_ifreq();
        ifr.ifr_ifru.ifru_netmask = self.to_sockaddr(netmask).into();
        if unsafe { ioctl(self.socket_tun, SIOCSIFNETMASK, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn enabled(&self, value: bool) -> Result<(), Error> {
        let mut ifr = self.set_get_ifreq();

        if unsafe { ioctl(self.socket_tun, SIOCSIFFLAGS, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }

        if value {
            unsafe {
                ifr.ifr_ifru.ifru_flags |= (IFF_UP | IFF_RUNNING) as c_short;
            }
        } else {
            unsafe {
                ifr.ifr_ifru.ifru_flags &= !(IFF_UP) as c_short;
            }
        }

        if unsafe { ioctl(self.socket_tun, SIOCSIFFLAGS, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }

        Ok(())
    }

    pub fn up(&self) -> Result<(), Error> {
        unsafe {
            let mut ifr = self.set_get_ifreq();

            if ioctl(self.socket_tun, SIOCSIFFLAGS, &ifr) < 0 {
                return Err(Error::IOCtl(io::Error::last_os_error()));
            }

            ifr.ifr_ifru.ifru_flags |= (IFF_UP | IFF_RUNNING) as c_short;

            if ioctl(self.socket_tun, SIOCSIFFLAGS, &ifr) < 0 {
                return Err(Error::IOCtl(io::Error::last_os_error()));
            }

            Ok(())
        }
    }
}
