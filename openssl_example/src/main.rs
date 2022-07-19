use openssl::ssl::{SslConnector, SslMethod};
use std::io::{Read, Write};
use std::net::TcpStream;

fn main() {
    let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();

    let stream = TcpStream::connect("www.baidu.com:443").unwrap();
    let mut stream = connector.connect("www.baidu.com", stream).unwrap();

    stream.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
    let mut res = vec![];
    stream.read_to_end(&mut res).unwrap();
    println!("{}", String::from_utf8_lossy(&res));
}
