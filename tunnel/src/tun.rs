use libc::{c_short, close, ifreq, ioctl, open, read, write, IFNAMSIZ, O_RDWR};

use crate::tunerror::Error;
use std::ffi::CString;
use std::io;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::{mem, ptr};

const TUNSETIFF: u64 = 0x400454ca;
const IFF_TUN: c_short = 0x0001;
const IFF_NO_PI: c_short = 0x1000;

#[derive(Default, Debug)]
pub struct TunSocket {
    fd: RawFd,
    name: String,
}

impl Drop for TunSocket {
    fn drop(&mut self) {
        unsafe {
            close(self.fd);
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

        ifr.ifr_ifru.ifru_flags = IFF_TUN | IFF_NO_PI;

        if unsafe { ioctl(fd, TUNSETIFF as _, &ifr) } < 0 {
            return Err(Error::IOCtl(io::Error::last_os_error()));
        }

        let name = dev.to_string();

        Ok(TunSocket { fd, name })
    }

    pub fn read(&self, dst: &mut [u8]) -> Result<usize, Error> {
        match unsafe { read(self.fd, dst.as_mut_ptr() as _, dst.len()) } {
            -1 => Err(Error::IfaceRead(io::Error::last_os_error())),
            n => Ok(n as usize),
        }
    }

    pub fn write(&self, buf: &[u8]) -> usize {
        match unsafe { write(self.fd, buf.as_ptr() as _, buf.len() as _) } {
            -1 => {
                println!("{:?}", Error::IfaceRead(io::Error::last_os_error()));
                0
            }
            n => n as usize,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
