use libp2p::{
  core::{
    muxing::StreamMuxerBox,
    transport::{ListenerId, TransportEvent},
  },
  futures::StreamExt,
  identity::Keypair,
  quic, Transport,
};

#[tokio::main]
async fn main() {
  let keypair = Keypair::generate_ed25519();
  let quic_config = quic::Config::new(&keypair);

  let mut quic_transport = quic::GenTransport::<quic::tokio::Provider>::new(quic_config)
    .map(|(p, c), _| (p, StreamMuxerBox::new(c)))
    .boxed();

  let addr = "/ip4/127.0.0.1/udp/12345/quic-v1"
    .parse()
    .expect("address should be valid");

  quic_transport
    .listen_on(ListenerId::next(), addr)
    .expect("listen error.");
  match quic_transport.next().await {
    Some(TransportEvent::NewAddress { .. }) => {
      // println!("listen_addr:{:?}", listen_addr)
      loop {
        let event = quic_transport.select_next_some().await;
        println!("upgrade, send_back_addr:{:?}", event);
      }
    }
    e => panic!("{e:?}"),
  }
}
