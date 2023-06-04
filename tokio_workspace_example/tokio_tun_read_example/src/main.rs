#[cfg(target_os = "linux")]
use std::net::Ipv4Addr;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
#[cfg(target_os = "linux")]
use tokio::io::AsyncReadExt;
#[cfg(target_os = "linux")]
use tokio_tun::{result::Result, TunBuilder};

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<()> {
    let tun = TunBuilder::new()
        .name("")
        .tap(false)
        .packet_info(false)
        .mtu(1350)
        .up()
        .address(Ipv4Addr::new(10, 0, 0, 1))
        .destination(Ipv4Addr::new(10, 1, 0, 1))
        .broadcast(Ipv4Addr::BROADCAST)
        .netmask(Ipv4Addr::new(255, 255, 255, 0))
        .try_build()?;

    println!("-----------");
    println!("tun created");
    println!("-----------");

    println!(
        "┌ name: {}\n├ fd: {}\n├ mtu: {}\n├ flags: {}\n├ address: {}\n├ destination: {}\n├ broadcast: {}\n└ netmask: {}",
        tun.name(),
        tun.as_raw_fd(),
        tun.mtu().unwrap(),
        tun.flags().unwrap(),
        tun.address().unwrap(),
        tun.destination().unwrap(),
        tun.broadcast().unwrap(),
        tun.netmask().unwrap(),
    );

    println!("---------------------");
    println!("ping 10.1.0.2 to test");
    println!("---------------------");

    let (mut reader, mut _writer) = tokio::io::split(tun);

    let mut buf = [0u8; 1024];
    loop {
        let n = reader.read(&mut buf).await?;
        println!("reading {} bytes: {:?}", n, &buf[..n]);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("hello world!");
}
