import struct
from fcntl import ioctl


def create_tun(name: str):
    tun = open("/dev/net/tun", "r+b", buffering=0)
    LINUX_IFF_TUN = 0x0001
    LINUX_IFF_NO_PI = 0x1000
    LINUX_TUNSETIFF = 0x400454CA
    flags = LINUX_IFF_TUN | LINUX_IFF_NO_PI
    ifs = struct.pack("16sH22s", name, flags, b"")
    ioctl(tun, LINUX_TUNSETIFF, ifs)
    return tun

if __name__ == "__main__":
    create_tun("playtun")
