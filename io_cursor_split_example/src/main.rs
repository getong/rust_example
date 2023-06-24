use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("localhost:8080").unwrap();

    // telnet localhost 8080

    while let Ok((mut connection, _)) = listener.accept() {
        std::thread::spawn(move || {
            loop {
                let mut buffer = [0u8; 9]; // Buffer size of 9 bytes

                // Read 9 bytes from the TCP stream
                connection.read_exact(&mut buffer).unwrap();

                // Create a cursor from the buffer
                let mut cursor = Cursor::new(buffer);

                // Read the first 1 byte from the cursor
                let mut first_part = [0u8; 1];
                cursor.read_exact(&mut first_part).unwrap();

                // Read the next 8 bytes from the cursor
                let mut second_part = [0u8; 8];
                cursor.read_exact(&mut second_part).unwrap();

                // Convert the variables from network byte order to host byte order
                let u8_value = first_part[0];
                let u64_value = u64::from_be_bytes(second_part);

                // Print the values
                println!("u8 value: {}", u8_value);
                println!("u64 value: {}", u64_value);

                // Print the first part of the buffer
                println!("First part: {:?}", String::from_utf8_lossy(&first_part));

                // Print the second part of the buffer
                println!("Second part: {:?}", String::from_utf8_lossy(&second_part));
                _ = connection.write_all(&first_part);
                _ = connection.write_all(&[b'\n']);
                _ = connection.write_all(&second_part);
            }
        });
    }
}
