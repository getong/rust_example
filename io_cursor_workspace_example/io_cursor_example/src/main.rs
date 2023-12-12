use std::io::{self, Cursor, Read, Write};

fn main() -> io::Result<()> {
  cursor_write_vec();
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

fn cursor_write_vec() {
  let mut buf = [0; 32];
  let mut cursor = Cursor::new(&mut buf[..]);
  _ = cursor.write(&[1, 2, 3]);
  _ = cursor.write(&[4, 5, 6]);
  assert_eq!(&buf[0..6], &[1, 2, 3, 4, 5, 6]);
}
