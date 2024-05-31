use std::{io, mem::MaybeUninit, os::fd::RawFd, ptr};

use libc::{c_int, fd_set, timeval, FD_CLR, FD_ISSET, FD_SET, FD_ZERO};

/// Implementation copied from https://blog.pjam.me/posts/select-syscall-in-rust/
pub struct FdSet(libc::fd_set);

impl FdSet {
    pub fn new() -> FdSet {
        unsafe {
            let mut raw_fd_set = MaybeUninit::<fd_set>::uninit();
            FD_ZERO(raw_fd_set.as_mut_ptr());
            FdSet(raw_fd_set.assume_init())
        }
    }

    pub fn clear(&mut self, fd: RawFd) {
        unsafe { FD_CLR(fd, &mut self.0) }
    }

    pub fn set(&mut self, fd: RawFd) {
        unsafe { FD_SET(fd, &mut self.0) }
    }

    pub fn is_set(&mut self, fd: RawFd) -> bool {
        unsafe { FD_ISSET(fd, &mut self.0) }
    }
}

fn to_fdset_ptr(opt: Option<&mut FdSet>) -> *mut fd_set {
    match opt {
        None => ptr::null_mut(),
        Some(&mut FdSet(ref mut raw_fd_set)) => raw_fd_set,
    }
}

fn to_ptr<T>(opt: Option<&T>) -> *const T {
    match opt {
        None => ptr::null::<T>(),
        Some(p) => p,
    }
}

pub fn select(
    nfds: c_int,
    readfds: Option<&mut FdSet>,
    writefds: Option<&mut FdSet>,
    errorfds: Option<&mut FdSet>,
    timeout: Option<&timeval>,
) -> io::Result<usize> {
    match unsafe {
        libc::select(
            nfds,
            to_fdset_ptr(readfds),
            to_fdset_ptr(writefds),
            to_fdset_ptr(errorfds),
            to_ptr::<timeval>(timeout) as *mut timeval,
        )
    } {
        -1 => Err(io::Error::last_os_error()),
        res => Ok(res as usize),
    }
}
