from get import get_tun


def read_tun(name: str):
    """
    Continuously read data from a tun device in a loop.
    """
    tun = get_tun(name, False)
    while True:
        data = tun.read(1024)
        print(f"Read {len(data)} bytes from device {name}")


if __name__ == "__main__":
    read_tun("playtun")
