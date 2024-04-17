import struct
from fcntl import ioctl


def delete_tun(name: str):
    """
    This function deletes a tun device. We can use the function to delete 
    an already existing tun device with name. If the tun device exists, its
    persistent attribute is unset. If the tun device does not exist,
    a tun device is created, butit's quickly removed automatically by the kernel
    once this program exits.
    """
    tun = open("/dev/net/tun", "r+b", buffering=0)
    LINUX_IFF_TUN = 0x0001
    LINUX_IFF_NO_PI = 0x1000
    LINUX_TUNSETIFF = 0x400454CA
    flags = LINUX_IFF_TUN | LINUX_IFF_NO_PI
    ifs = struct.pack("16sH22s", name, flags, b"")
    ioctl(tun, LINUX_TUNSETIFF, ifs)
    LINUX_TUNSETPERSIST = 0x400454CB
    ioctl(tun, LINUX_TUNSETPERSIST, 0)

if __name__ == "__main__":
    tun_fd = delete_tun(b"playtun")
    print(f"Tun device playtun has been deleted")
