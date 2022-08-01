
use std::{net::TcpStream, io::{Write, Read}};

fn test_delay_server() {
    let mut stream = TcpStream::connect("127.0.0.1:8080").unwrap();
    println!("written bytes: {}", stream.write(b"Delay 1").unwrap());

    let mut buf = [0;1500];
    println!("read bytes: {}", stream.read(&mut buf).unwrap());

    stream.shutdown(std::net::Shutdown::Both).unwrap();
}

fn main() {
    test_delay_server();
}

