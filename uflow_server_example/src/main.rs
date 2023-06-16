fn main() {
    let server_address = "127.0.0.1:8888";
    let config = Default::default();

    // Create a server object
    let mut server = uflow::server::Server::bind(server_address, config).unwrap();

    loop {
        // Process inbound UDP frames and handle events
        for event in server.step() {
            match event {
                uflow::server::Event::Connect(client_address) => {
                    println!("[{:?}] connected", client_address);
                }
                uflow::server::Event::Disconnect(client_address) => {
                    println!("[{:?}] disconnected", client_address);
                }
                uflow::server::Event::Error(client_address, err) => {
                    println!("[{:?}] error: {:?}", client_address, err);
                }
                uflow::server::Event::Receive(client_address, packet_data) => {
                    let packet_data_utf8 = std::str::from_utf8(&packet_data).unwrap();
                    let reversed_string: std::string::String = packet_data_utf8.chars().rev().collect();

                    println!("[{:?}] received \"{}\"", client_address, packet_data_utf8);

                    let mut client = server.client(&client_address).unwrap().borrow_mut();

                    // Echo the packet reliably on channel 0
                    client.send(packet_data, 0, uflow::SendMode::Reliable);

                    // Echo the reverse of the packet unreliably on channel 1
                    client.send(reversed_string.as_bytes().into(), 1, uflow::SendMode::Unreliable);
                }
            }
        }

        // Flush outbound UDP frames
        server.flush();

        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
