use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result, anyhow};
use bevy::prelude::*;
use quinn::crypto::rustls::QuicClientConfig;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio::{runtime::Runtime, sync::mpsc};

use crate::{
  ClientWorld,
  protocol::{
    ALPN_PROTOCOL, ClientEnvelope, DEFAULT_SERVER_ADDR, Hello, ServerEnvelope, client_envelope,
    read_server_envelope, server_envelope, write_client_envelope,
  },
};

#[allow(dead_code)]
#[derive(Resource)]
pub(crate) struct NetworkRuntime(Runtime);

#[derive(Resource)]
pub(crate) struct NetworkEvents {
  receiver: mpsc::UnboundedReceiver<NetworkEvent>,
}

#[derive(Resource)]
pub(crate) struct NetworkClient {
  pub(crate) sender: mpsc::UnboundedSender<ClientEnvelope>,
  pub(crate) sequence: u64,
  pub(crate) connected: bool,
}

pub(crate) enum NetworkEvent {
  Connected,
  Message(ServerEnvelope),
  Disconnected(String),
}

pub(crate) fn start_network_client(mut commands: Commands) {
  let (event_sender, event_receiver) = mpsc::unbounded_channel();
  let (command_sender, command_receiver) = mpsc::unbounded_channel();
  let runtime = Runtime::new().expect("tokio runtime should start");

  runtime.spawn(async move {
    if let Err(err) =
      run_quic_client(DEFAULT_SERVER_ADDR, event_sender.clone(), command_receiver).await
    {
      let _ = event_sender.send(NetworkEvent::Disconnected(format!("{err:#}")));
    }
  });

  commands.insert_resource(NetworkRuntime(runtime));
  commands.insert_resource(NetworkEvents {
    receiver: event_receiver,
  });
  commands.insert_resource(NetworkClient {
    sender: command_sender,
    sequence: 0,
    connected: false,
  });
}

pub(crate) fn drain_network_events(
  mut events: ResMut<NetworkEvents>,
  mut network: ResMut<NetworkClient>,
  mut world: ResMut<ClientWorld>,
) {
  while let Ok(event) = events.receiver.try_recv() {
    match event {
      NetworkEvent::Connected => {
        network.connected = true;
        world.status = "connected".to_string();
      }
      NetworkEvent::Message(message) => {
        handle_server_message(message, &mut world);
      }
      NetworkEvent::Disconnected(reason) => {
        network.connected = false;
        world.status = format!("disconnected: {reason}");
      }
    }
  }
}

fn handle_server_message(message: ServerEnvelope, world: &mut ClientWorld) {
  match message.payload {
    Some(server_envelope::Payload::Welcome(welcome)) => {
      world.local_actor_id = Some(welcome.actor_id);
      world.tick = welcome.tick;
      world.status = format!("connected as client {}", welcome.client_id);
    }
    Some(server_envelope::Payload::Snapshot(snapshot)) => {
      world.tick = snapshot.tick;
      if let Some(map) = snapshot.map {
        world.map = Some(map);
      }
      world.actors = snapshot
        .actors
        .into_iter()
        .map(|actor| (actor.id, actor))
        .collect();
    }
    Some(server_envelope::Payload::Pong(pong)) => {
      world.tick = pong.server_tick;
    }
    Some(server_envelope::Payload::Notice(notice)) => {
      world.status = notice.message;
    }
    None => {}
  }
}

async fn run_quic_client(
  server_addr: &str,
  event_sender: mpsc::UnboundedSender<NetworkEvent>,
  command_receiver: mpsc::UnboundedReceiver<ClientEnvelope>,
) -> Result<()> {
  let remote = server_addr
    .parse::<SocketAddr>()
    .with_context(|| format!("invalid server address {server_addr}"))?;
  let mut endpoint = quinn::Endpoint::client("[::]:0".parse::<SocketAddr>()?)
    .context("failed to bind quinn client endpoint")?;
  endpoint.set_default_client_config(client_config()?);

  let connection = endpoint
    .connect(remote, "localhost")
    .context("failed to start quinn connection")?
    .await
    .map_err(|err| anyhow!("failed to connect to server: {err}"))?;
  let (mut send, recv) = connection
    .open_bi()
    .await
    .context("failed to open client stream")?;

  event_sender
    .send(NetworkEvent::Connected)
    .context("failed to publish connection event")?;

  let hello = ClientEnvelope {
    payload: Some(client_envelope::Payload::Hello(Hello {
      name: "bevy-client".to_string(),
    })),
  };
  write_client_envelope(&mut send, &hello).await?;

  let reader = tokio::spawn(read_loop(recv, event_sender.clone()));
  let writer = tokio::spawn(write_loop(send, command_receiver));

  tokio::select! {
    result = reader => {
      result.context("client reader task panicked")??;
    }
    result = writer => {
      result.context("client writer task panicked")??;
    }
  }

  endpoint.wait_idle().await;
  Ok(())
}

async fn read_loop(
  mut recv: quinn::RecvStream,
  event_sender: mpsc::UnboundedSender<NetworkEvent>,
) -> Result<()> {
  while let Some(message) = read_server_envelope(&mut recv).await? {
    event_sender
      .send(NetworkEvent::Message(message))
      .context("failed to publish server message")?;
  }
  Ok(())
}

async fn write_loop(
  mut send: quinn::SendStream,
  mut receiver: mpsc::UnboundedReceiver<ClientEnvelope>,
) -> Result<()> {
  while let Some(message) = receiver.recv().await {
    write_client_envelope(&mut send, &message).await?;
  }
  Ok(())
}

fn client_config() -> Result<quinn::ClientConfig> {
  let mut client_crypto = rustls::ClientConfig::builder()
    .dangerous()
    .with_custom_certificate_verifier(SkipServerVerification::new())
    .with_no_client_auth();
  client_crypto.alpn_protocols = vec![ALPN_PROTOCOL.to_vec()];

  Ok(quinn::ClientConfig::new(Arc::new(
    QuicClientConfig::try_from(client_crypto).context("failed to build quic client config")?,
  )))
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
  fn new() -> Arc<Self> {
    Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
  }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
  fn verify_server_cert(
    &self,
    _end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp: &[u8],
    _now: UnixTime,
  ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
    Ok(rustls::client::danger::ServerCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls12_signature(
      message,
      cert,
      dss,
      &self.0.signature_verification_algorithms,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls13_signature(
      message,
      cert,
      dss,
      &self.0.signature_verification_algorithms,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.0.signature_verification_algorithms.supported_schemes()
  }
}
