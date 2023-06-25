use std::io::{self, Cursor, Read};

fn main() -> io::Result<()> {
    // Simulating network traffic data
    let network_data: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04, 0x05];

    // Create a cursor from the network data
    let mut cursor = Cursor::new(network_data);

    // Read and process the network data
    let mut buffer = [0u8; 2];
    loop {
        match cursor.read(&mut buffer) {
            Ok(0) => break, // Reached the end of the network data
            Ok(bytes_read) => {
                // Process the received bytes
                process_received_bytes(&buffer[..bytes_read])?;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

fn process_received_bytes(bytes: &[u8]) -> io::Result<()> {
    // Process the received bytes
    // Example: Print each received byte
    for byte in bytes {
        println!("Received byte: {}", byte);
    }

    Ok(())
}
