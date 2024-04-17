import struct
from fcntl import ioctl


def get_tun(name: str, is_create: bool):
    """
    This function gets/creates a tun device. We can use the function to get 
    an already existing tun device with name. If the device does not exist and if 
    `is_create` option is set, it creates a persistent tun device. If `is_create` option
    is not set to true and the tun device does not exist, a tun device is created, but
    it's quickly removed automatically by the kernel once this program exits.
    """
    tun = open("/dev/net/tun", "r+b", buffering=0) # Open the clone device.
    LINUX_IFF_TUN = 0x0001 # We want a tun device
    LINUX_IFF_NO_PI = 0x1000 # We don't want packet information
    LINUX_TUNSETIFF = 0x400454CA # Create tun device with {name} argument if it doesn't exist.
    flags = LINUX_IFF_TUN | LINUX_IFF_NO_PI
    ifs = struct.pack("16sH22s", name.encode("utf-8"), flags, b"")
    ioctl(tun, LINUX_TUNSETIFF, ifs)
    if is_create:
        # If we're creating a tun, we want it to be persistent.
        LINUX_TUNSETPERSIST = 0x400454CB
        ioctl(tun, LINUX_TUNSETPERSIST, 1)
    return tun

if __name__ == "__main__":
    _ = get_tun("playtun", True)
