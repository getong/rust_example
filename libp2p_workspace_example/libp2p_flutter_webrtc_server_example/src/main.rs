use std::{collections::BTreeSet, net::Ipv4Addr, time::Duration};

use anyhow::Result;
use futures::StreamExt;
use libp2p::{
  core::muxing::StreamMuxerBox,
  multiaddr::{Multiaddr, Protocol},
  request_response::{self, ProtocolSupport},
  swarm::{NetworkBehaviour, SwarmEvent},
  StreamProtocol, Transport,
};
use libp2p_webrtc as webrtc;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatRequest {
  message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChatResponse {
  original_message: String,
  echoed_message: String,
  server_timestamp: String,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
  request_response: request_response::cbor::Behaviour<ChatRequest, ChatResponse>,
}

#[tokio::main]
async fn main() -> Result<()> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(
      "libp2p_flutter_webrtc_server_example=info,libp2p_webrtc=info,libp2p_request_response=info",
    )
    .try_init();

  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_other_transport(|id_keys| {
      Ok(
        webrtc::tokio::Transport::new(
          id_keys.clone(),
          webrtc::tokio::Certificate::generate(&mut thread_rng())?,
        )
        .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn))),
      )
    })?
    .with_behaviour(|_| Behaviour {
      request_response: request_response::cbor::Behaviour::new(
        [(
          StreamProtocol::new("/flutter-chat/1"),
          ProtocolSupport::Full,
        )],
        request_response::Config::default(),
      ),
    })?
    .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
    .build();

  let listen_addr = Multiaddr::from(Ipv4Addr::UNSPECIFIED)
    .with(Protocol::Udp(0))
    .with(Protocol::WebRTCDirect);

  swarm.listen_on(listen_addr)?;

  let local_peer_id = *swarm.local_peer_id();
  let mut announced = BTreeSet::new();

  tracing::info!(peer_id=%local_peer_id, "Flutter-targeted libp2p WebRTC chat server started");
  tracing::info!(
    "Paste one of the printed /webrtc-direct/certhash/.../p2p/... addresses into the Flutter \
     client page."
  );

  loop {
    match swarm.select_next_some().await {
      SwarmEvent::NewListenAddr { address, .. } => {
        let dial_addr = address.with(Protocol::P2p(local_peer_id));
        if announced.insert(dial_addr.to_string()) {
          tracing::info!(multiaddr=%dial_addr, "Dial address");
        }
      }
      SwarmEvent::ConnectionEstablished {
        peer_id, endpoint, ..
      } => {
        tracing::info!(%peer_id, ?endpoint, "Connection established");
      }
      SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
        tracing::info!(%peer_id, ?cause, "Connection closed");
      }
      SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
        request_response::Event::Message {
          peer,
          message: request_response::Message::Request {
            request, channel, ..
          },
          ..
        },
      )) => {
        let server_timestamp = current_timestamp_label();
        let response = ChatResponse {
          original_message: request.message.clone(),
          echoed_message: format!("{} [{}]", request.message, server_timestamp),
          server_timestamp,
        };

        tracing::info!(%peer, original=%request.message, echoed=%response.echoed_message, "Received chat request");

        swarm
          .behaviour_mut()
          .request_response
          .send_response(channel, response)
          .expect("response channel should stay open while handling request");
      }
      SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
        request_response::Event::ResponseSent { peer, .. },
      )) => {
        tracing::info!(%peer, "Chat response sent");
      }
      SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
        request_response::Event::InboundFailure { peer, error, .. },
      )) => {
        tracing::warn!(%peer, ?error, "Inbound request failed");
      }
      SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
        request_response::Event::OutboundFailure { peer, error, .. },
      )) => {
        tracing::warn!(%peer, ?error, "Unexpected outbound failure");
      }
      event => tracing::debug!(?event, "Swarm event"),
    }
  }
}

fn current_timestamp_label() -> String {
  use std::time::{SystemTime, UNIX_EPOCH};

  match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(duration) => format!(
      "unix:{}.{:03}",
      duration.as_secs(),
      duration.subsec_millis()
    ),
    Err(_) => "unix:unknown".to_owned(),
  }
}
