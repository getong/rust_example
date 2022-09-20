#![feature(never_type)]

use socket2::{Domain, Socket, Type};
use std::net::{SocketAddr, TcpListener};

fn main() -> Result<!, Box<dyn std::error::Error>> {
    // Create a TCP listener bound to two addresses.
    let socket = Socket::new(Domain::IPV6, Type::STREAM, None)?;

    socket.set_only_v6(false)?;
    let address: SocketAddr = "[::1]:12345".parse().unwrap();
    socket.bind(&address.into())?;
    socket.listen(128)?;

    let _listener: TcpListener = socket.into();

    // ...
    loop {}
    //Ok(())
}
