use laminar::{Packet, Socket};
use std::error::Error;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    // println!("Hello, world!");
    // Creates the socket
    let mut socket = Socket::bind_any()?;
    let packet_sender = socket.get_packet_sender();
    // Starts the socket, which will start a poll mechanism to receive and send messages.
    let _thread = thread::spawn(move || socket.start_polling());

    // Bytes to sent
    let bytes1 = b"hello world!".to_vec();
    let bytes2 = bytes1.clone();
    let bytes3 = bytes1.clone();
    let bytes4 = bytes1.clone();
    let bytes5 = bytes1.clone();
    let destination = "127.0.0.1:12346".parse()?;

    // Creates packets with different reliabilities
    let unreliable = Packet::unreliable(destination, bytes1);
    let reliable = Packet::reliable_unordered(destination, bytes2);

    // Specifies on which stream and how to order our packets, check out our book and documentation for more information
    let unreliable_sequenced = Packet::unreliable_sequenced(destination, bytes3, Some(1));
    let reliable_sequenced = Packet::reliable_sequenced(destination, bytes4, Some(2));
    let reliable_ordered = Packet::reliable_ordered(destination, bytes5, Some(3));

    // Sends the created packets
    packet_sender.send(unreliable).unwrap();
    packet_sender.send(reliable).unwrap();
    packet_sender.send(unreliable_sequenced).unwrap();
    packet_sender.send(reliable_sequenced).unwrap();
    packet_sender.send(reliable_ordered).unwrap();
    thread::sleep(Duration::from_millis(5));
    Ok(())
}
