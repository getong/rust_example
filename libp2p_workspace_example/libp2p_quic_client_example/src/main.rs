use libp2p::Transport;
use libp2p_quic as quic;

#[tokio::main]
async fn main() {
  let keypair = libp2p_identity::Keypair::generate_ed25519();
  let quic_config = quic::Config::new(&keypair);

  let mut quic_transport = quic::tokio::Transport::new(quic_config);

  let addr = "/ip4/127.0.0.1/udp/12345/quic-v1"
    .parse()
    .expect("address should be valid");

  quic_transport.dial(addr).expect("listen error.");
}
