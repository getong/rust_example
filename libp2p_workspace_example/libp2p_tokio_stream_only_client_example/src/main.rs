use std::{io, time::Duration};

use anyhow::{Context, Result};
use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::{identity, multiaddr::Protocol, Multiaddr, PeerId, Stream, StreamProtocol};
use libp2p_stream as stream;
use rand::RngCore;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

const ECHO_PROTOCOL: StreamProtocol = StreamProtocol::new("/echo");

// no listen_on
// cargo run  --
// /ip4/127.0.0.1/udp/64663/quic-v1/p2p/12D3KooWAQ3rC8s13mpmiEXQbGcJdvf7EPFnhMh5yYSGHVjYpxWy

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_env_filter(
      EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?,
    )
    .init();

  let maybe_address = std::env::args()
    .nth(1)
    .map(|arg| arg.parse::<Multiaddr>())
    .transpose()
    .context("Failed to parse argument as `Multiaddr`")?;

  // todo
  let hex_data = hex::decode("080112405862d6fe84c9530590f8b825a47d6965d925f0469dc0be7b239909bd2c3e787750846cb48ac74b2205d2913ce1e4211aa69f5609eae72c81492170981c6e0cc7").unwrap();
  let keypair = identity::Keypair::from_protobuf_encoding(&hex_data).unwrap();

  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
    .with_tokio()
    .with_quic()
    .with_behaviour(|_| stream::Behaviour::new())?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10)))
    .build();
  // swarm.listen_on("/ip4/0.0.0.0/udp/5500/quic-v1".parse()?)?;
  // println!("swarm is {:?}", swarm.network_info());

  let mut incoming_streams = swarm
    .behaviour()
    .new_control()
    .accept(ECHO_PROTOCOL)
    .unwrap();

  // Deal with incoming streams.
  // Spawning a dedicated task is just one way of doing this.
  // libp2p doesn't care how you handle incoming streams but you _must_ handle them somehow.
  // To mitigate DoS attacks, libp2p will internally drop incoming streams if your application
  // cannot keep up processing them.
  tokio::spawn(async move {
    // This loop handles incoming streams _sequentially_ but that doesn't have to be the case.
    // You can also spawn a dedicated task per stream if you want to.
    // Be aware that this breaks backpressure though as spawning new tasks is equivalent to an
    // unbounded buffer. Each task needs memory meaning an aggressive remote peer may force you
    // OOM this way.

    while let Some((peer, stream)) = incoming_streams.next().await {
      match echo(stream).await {
        Ok(n) => {
          tracing::info!(%peer, "Echoed {n} bytes!");
        }
        Err(e) => {
          tracing::warn!(%peer, "Echo failed: {e}");
          continue;
        }
      };
    }
  });

  // In this demo application, the dialing peer initiates the protocol.
  if let Some(address) = maybe_address {
    let Some(Protocol::P2p(peer_id)) = address.iter().last() else {
      anyhow::bail!("Provided address does not end in `/p2p`");
    };

    swarm.dial(address)?;

    tokio::spawn(connection_handler(peer_id, swarm.behaviour().new_control()));
  }

  // Poll the swarm to make progress.
  loop {
    let event = swarm.next().await.expect("never terminates");

    match event {
      libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
        let listen_address = address.with_p2p(*swarm.local_peer_id()).unwrap();
        // println!("listen_address: {:?}", listen_address);
        tracing::info!(%listen_address);
      }
      event => tracing::trace!(?event),
    }
  }
}

/// A very simple, `async fn`-based connection handler for our custom echo protocol.
async fn connection_handler(peer: PeerId, mut control: stream::Control) {
  loop {
    tokio::time::sleep(Duration::from_secs(1)).await; // Wait a second between echos.

    let stream = match control.open_stream(peer, ECHO_PROTOCOL).await {
      Ok(stream) => stream,
      Err(error @ stream::OpenStreamError::UnsupportedProtocol(_)) => {
        tracing::info!(%peer, %error);
        return;
      }
      Err(error) => {
        // Other errors may be temporary.
        // In production, something like an exponential backoff / circuit-breaker may be more
        // appropriate.
        tracing::debug!(%peer, %error);
        continue;
      }
    };

    if let Err(e) = send(stream).await {
      tracing::warn!(%peer, "Echo protocol failed: {e}");
      continue;
    }

    tracing::info!(%peer, "Echo complete!")
  }
}

async fn echo(mut stream: Stream) -> io::Result<usize> {
  let mut total = 0;

  let mut buf = [0u8; 100];

  loop {
    let read = stream.read(&mut buf).await?;
    if read == 0 {
      return Ok(total);
    }

    total += read;
    println!("buf[..read] is {:?}", &buf[.. read]);
    stream.write_all(&buf[.. read]).await?;
  }
}

async fn send(mut stream: Stream) -> io::Result<()> {
  let num_bytes = rand::random::<usize>() % 1000;

  let mut bytes = vec![0; num_bytes];
  rand::thread_rng().fill_bytes(&mut bytes);

  stream.write_all(&bytes).await?;

  let mut buf = vec![0; num_bytes];
  stream.read_exact(&mut buf).await?;

  if bytes != buf {
    return Err(io::Error::new(io::ErrorKind::Other, "incorrect echo"));
  }
  println!("bytes is {:?}", &bytes);
  stream.close().await?;

  Ok(())
}
