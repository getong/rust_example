use laminar::{Socket, SocketEvent};
use std::error::Error;
//use std::net::SocketAddr;
use std::thread;

fn main() -> Result<(), Box<dyn Error>> {
  // Creates the socket
  let mut socket = Socket::bind("127.0.0.1:12346")?;
  let event_receiver = socket.get_event_receiver();
  // Starts the socket, which will start a poll mechanism to receive and send messages.
  let _thread = thread::spawn(move || socket.start_polling());

  // Waits until a socket event occurs
  loop {
    match event_receiver.recv() {
      Ok(socket_event) => {
        println!("socket_event:{:?}", socket_event);

        match socket_event {
          SocketEvent::Packet(packet) => {
            let received_data: &[u8] = packet.payload();
            println!(
              "received_data raw bytes : {:?}, try to convert into string: {:?}",
              received_data,
              String::from_utf8_lossy(received_data)
            );
          }
          SocketEvent::Connect(connect_event) => {
            println!("connect_event:{:?}", connect_event);
          }
          SocketEvent::Timeout(timeout_event) => {
            println!("timeout_event:{:?}", timeout_event);
          }
          SocketEvent::Disconnect(disconnect_event) => {
            println!("disconnect_event:{:?}", disconnect_event);
          }
        }
      }
      Err(e) => {
        println!("Something went wrong when receiving, error: {:?}", e);
      }
    }
  }
}
