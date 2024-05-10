from ctypes import *
from ctypes.util import find_library
import socket
import struct

from get import get_tun


def dump_tun(name: str):
    tun = get_tun(name, False)
    while True:
        data = tun.read(1024)
        if len(data) > 0:
            dump_packet(data)


def dump_packet(packet):
    version = packet[0] >> 4
    if version == 4:
        dump_ipv4_packet(packet)
    elif version == 6:
        dump_ipv6_packet(packet)
    else:
        print("Unknown packet version")


def dump_ipv4_packet(packet):
    if len(packet) < 20:
        print("IPv4 packet is too short")
        return

    protocol = packet[9]
    protocol_name = getprotobynumber(protocol)
    protocol_name = "?" if protocol_name == "" else protocol_name

    ttl = packet[8]
    args = tuple(p for p in packet[12:20]) + (protocol, protocol_name, ttl, len(packet))
    print("IPv4: src=%d.%d.%d.%d dst=%d.%d.%d.%d proto=%d(%s) ttl=%d len=%d" % args)
    dump_ports(protocol, packet[20:])
    print(f" HEX: {packet.hex()}")


def dump_ipv6_packet(packet):
    if len(packet) < 40:
        print("IPv6 packet is too short")
        return

    protocol = packet[6]
    protocol_name = getprotobynumber(protocol)
    protocol_name = "?" if protocol_name == "" else protocol_name

    hop_limit = packet[7]

    source_address = packet[8:24].hex()
    dest_address = packet[24:40].hex()
    print(
        f"IPv6 src={source_address} dst={dest_address} proto={protocol}({protocol_name}) hop_limit={hop_limit} len={len(packet)}"
    )
    dump_ports(protocol, packet[40:])
    print(f" HEX: {packet.hex()}")


def dump_ports(protocol, buffer):
    if not has_port(protocol):
        return

    if len(buffer) < 4:
        return

    source_port, dest_port = struct.unpack(">HH", buffer[:4])
    print(f" sport={source_port}, dport={dest_port}")


def has_port(protocol):
    match protocol:
        case socket.IPPROTO_UDP | socket.IPPROTO_TCP:
            return True
        case _:
            return False


class ProtoEntry(Structure):
    _fields_ = (
        ("p_name", c_char_p),
        ("p_aliases", POINTER(c_char_p)),
        ("p_proto", c_int),
    )


def getprotobynumber(protocol: int) -> str:
    libc = cdll.LoadLibrary(find_library("c"))
    getprotobynumber = libc.getprotobynumber
    getprotobynumber.restype = POINTER(ProtoEntry)
    pe = getprotobynumber(protocol)
    if bool(pe) is False:
        return ""
    return bytes.decode(pe.contents.p_name, "utf-8")


if __name__ == "__main__":
    dump_tun("playtun")
