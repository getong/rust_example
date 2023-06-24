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
                let mut buffer = [0u8; 7]; // Buffer size of 7 bytes

                // Read 7 bytes from the TCP stream
                connection.read_exact(&mut buffer).unwrap();

                // Create a cursor from the buffer
                let mut cursor = Cursor::new(buffer);

                // Read the first 2 bytes from the cursor
                let mut first_part = [0u8; 2];
                cursor.read_exact(&mut first_part).unwrap();

                // Read the next 5 bytes from the cursor
                let mut second_part = [0u8; 5];
                cursor.read_exact(&mut second_part).unwrap();

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
