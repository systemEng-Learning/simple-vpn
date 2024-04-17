import struct
from fcntl import ioctl


def get_tun(name: str, is_create: bool):
    tun = open("/dev/net/tun", "r+b", buffering=0) # Open the clone device.
    LINUX_IFF_TUN = 0x0001 # We want a tun device
    LINUX_IFF_NO_PI = 0x1000
    LINUX_TUNSETIFF = 0x400454CA
    flags = LINUX_IFF_TUN | LINUX_IFF_NO_PI
    ifs = struct.pack("16sH22s", name.encode("utf-8"), flags, b"")
    ioctl(tun, LINUX_TUNSETIFF, ifs)
    if is_create:
        LINUX_TUNSETPERSIST = 0x400454CB
        ioctl(tun, LINUX_TUNSETPERSIST, 1)
    return tun

if __name__ == "__main__":
    _ = get_tun("playtun", True)
