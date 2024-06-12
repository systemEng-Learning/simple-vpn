use etherparse::{PacketBuilder, Ipv4HeaderSlice};

pub fn create_handshake_packet(ip_addr: &[u8; 4]) -> Vec<u8> {
    let builder = PacketBuilder::
        ipv4(ip_addr.clone(), [0, 0, 0, 0], 10)
        .udp(1, 1);
    let payload = [1];
    let mut result = Vec::<u8>::with_capacity(builder.size(payload.len()));
    builder.write(&mut result, &payload).unwrap();
    return result
}

pub fn is_handshake_packet(buf: &[u8]) -> bool {
    let slice = Ipv4HeaderSlice::from_slice(&buf);
                if slice.is_err() {
                    println!("{:?}", slice.err().unwrap());
                    return false;
                }
    slice.unwrap().destination_addr().is_unspecified()
}