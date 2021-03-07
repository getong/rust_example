extern crate clap;

use clap::{App, Arg};
use rand;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use trust_dns_client::op::{Message, MessageType, OpCode, Query};
use trust_dns_client::rr::domain::Name;
use trust_dns_client::rr::record_type::RecordType;
use trust_dns_client::serialize::binary::*;

fn main() {
    let app = App::new("resolve")
        .about("A simple to use DNS resolver")
        .arg(
            Arg::with_name("dns-server")
                .short("s")
                .default_value("1.1.1.1"),
        )
        .arg(Arg::with_name("domain-name").required(true))
        .get_matches();

    let domain_name_raw = app.value_of("domain-name").unwrap();
    let dns_server_raw = app.value_of("dns-server").unwrap();
    let mut request_as_bytes: Vec<u8> = Vec::with_capacity(512);
    let mut response_as_bytes: [u8; 512] = [0; 512];
    let domain_name = Name::from_ascii(&domain_name_raw).unwrap();
    let mut msg = Message::new();
    msg.set_id(rand::random::<u16>())
        .set_message_type(MessageType::Query)
        .add_query(Query::query(domain_name, RecordType::A))
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true);

    let mut encoder = BinEncoder::new(&mut request_as_bytes);
    msg.emit(&mut encoder).unwrap();
    let localhost = UdpSocket::bind("0.0.0.0:0").expect("cannot bind to local socket");
    let timeout = Duration::from_secs(3);
    localhost.set_read_timeout(Some(timeout)).unwrap();
    localhost.set_nonblocking(false).unwrap();

    let dns_server: SocketAddr = format!("{}:53", dns_server_raw)
        .parse()
        .expect("invalid address");
    let _amt = localhost
        .send_to(&request_as_bytes, dns_server)
        .expect("socket misconfigured");
    let (_amt, _remote) = localhost
        .recv_from(&mut response_as_bytes)
        .expect("timeout reached");

    let dns_message = Message::from_vec(&response_as_bytes).expect("unable to parse response");
    for answer in dns_message.answers() {
        if answer.record_type() == RecordType::A {
            let resource = answer.rdata();
            let ip = resource.to_ip_addr().expect("invalid IP address received");
            println!("{}", ip.to_string());
        }
    }
}
