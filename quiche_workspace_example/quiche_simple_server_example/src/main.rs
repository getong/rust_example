use quiche::ConnectionId;
use std::net::UdpSocket;

#[tokio::main]
async fn main() {
    let mut buf = [0; 65535];
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
    // config.set_application_protos(&[b"example-proto"]);

    // Create a new connection ID
    let conn_id = ConnectionId::from(vec![1, 2, 3, 4]);
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let local_addr = socket.local_addr().unwrap();
    // Server connection.
    let mut conn = quiche::accept(&conn_id, None, local_addr, local_addr, &mut config).unwrap();

    loop {
        let (read, from) = socket.recv_from(&mut buf).unwrap();

        let recv_info = quiche::RecvInfo {
            from: local_addr,
            to: from,
        };

        let _read = match conn.recv(&mut buf[..read], recv_info) {
            Ok(v) => v,

            Err(quiche::Error::Done) => {
                // Done reading.
                break;
            }

            Err(_e) => {
                // An error occurred, handle it.
                break;
            }
        };
    }
}
