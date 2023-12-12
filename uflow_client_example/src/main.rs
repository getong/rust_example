fn main() {
  let server_address = "127.0.0.1:8888";
  let config = Default::default();

  // Create a client object
  let mut client = uflow::client::Client::connect(server_address, config).unwrap();

  let mut send_counter = 0;
  let mut message_counter = 0;

  loop {
    // Process inbound UDP frames and handle events
    for event in client.step() {
      match event {
        uflow::client::Event::Connect => {
          println!("connected to server");
        }
        uflow::client::Event::Disconnect => {
          println!("disconnected from server");
        }
        uflow::client::Event::Error(err) => {
          println!("server connection error: {:?}", err);
        }
        uflow::client::Event::Receive(packet_data) => {
          let packet_data_utf8 = std::str::from_utf8(&packet_data).unwrap();

          println!("received \"{}\"", packet_data_utf8);
        }
      }
    }

    // Periodically send incrementing hello worlds on channel 0
    send_counter += 1;

    if send_counter == 10 {
      let packet_data: Box<[u8]> = format!("Hello world {}!", message_counter)
        .as_bytes()
        .into();
      let channel_id = 0;
      let send_mode = uflow::SendMode::Unreliable;

      client.send(packet_data, channel_id, send_mode);

      send_counter = 0;
      message_counter += 1;
    }

    // Flush outbound UDP frames
    client.flush();

    std::thread::sleep(std::time::Duration::from_millis(30));
  }
}
