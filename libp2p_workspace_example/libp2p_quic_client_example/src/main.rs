use libp2p::{
  core::{
    transport::{DialOpts, PortUse},
    Endpoint,
  },
  identity::Keypair,
  quic, Transport,
};

#[tokio::main]
async fn main() {
  let keypair = Keypair::generate_ed25519();
  let quic_config = quic::Config::new(&keypair);

  let mut quic_transport = quic::tokio::Transport::new(quic_config);

  let addr = "/ip4/127.0.0.1/udp/12345/quic-v1"
    .parse()
    .expect("address should be valid");

  _ = quic_transport
    .dial(
      addr,
      DialOpts {
        port_use: PortUse::Reuse,
        role: Endpoint::Dialer,
      },
    )
    .expect("listen error.")
    .await;
}
