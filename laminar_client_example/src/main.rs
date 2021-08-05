use laminar::{Packet, Socket};

use std::time::Instant;
fn main() {
    let mut client = Socket::bind_any().unwrap();
    let server_addr = "127.0.0.1:6666".parse().unwrap();

    client
        .send(Packet::reliable_unordered(server_addr, b"Hello!".to_vec()))
        .expect("This should send");
    client.manual_poll(Instant::now());
}
