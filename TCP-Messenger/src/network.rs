use std::net::TcpListener;

use std::io::{Read, Write};

use std::fs::File;
use std::thread;

pub struct NetworkHandler {
    listener: TcpListener,
}

impl NetworkHandler {
    pub fn new(ip_port: &str) -> Self {
        let listener = TcpListener::bind(ip_port).unwrap();

        NetworkHandler { listener: listener }
    }

    pub fn listen(self) {
        thread::spawn(move || {
            for stream in self.listener.incoming() {
                let mut buf = vec![];
                match stream {
                    Ok(mut value) => {
                        let raw = value.read_to_end(&mut buf);
                        match raw {
                            Ok(_) => {}
                            _ => eprintln!("somthing messed up inside raw"),
                        }
                    }
                    _ => eprintln!("something messup inside value"),
                }
                NetworkHandler::respond(&mut buf);
            }
        });
    }

    pub fn respond(raw: &mut Vec<u8>) {
        println!("Handled: {:?}", raw);
        raw.push(10);
        let mut file = File::options().append(true).open("/tmp/chat.txt").unwrap();
        file.write_all(&raw[..]).unwrap();
        println!("what the thell");
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn addrs_check() {
//         let ip = "127.0.0.1:9000";
//         let handler = NetworkHandler::new(ip);
//         let tmp_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127,0,0,1), 9000));
//         assert_eq!(handler.listener.local_addr().unwrap(), tmp_addr);
//     }
//     #[test]
//     fn write_to_stream() {
//         let handler = NetworkHandler::new("127.0.0.1:9000");
//         handler.listen();
//         match TcpStream::connect("127.0.0.1:9000") {
//             Ok(mut stream) => {
//                 stream.write(&"test".as_bytes()).unwrap();
//             },
//             _ => {
//                 eprintln!("couldnt connect to ip:port");

//             }

//         };
//         match TcpStream::connect("127.0.0.1:9000") {
//             Ok(mut stream) => {
//                 stream.write(&"how are you?".as_bytes()).unwrap();
//             },
//             _ => {
//                 eprintln!("couldnt connect to ip:port");

//             }

//         };

//     }
// }
