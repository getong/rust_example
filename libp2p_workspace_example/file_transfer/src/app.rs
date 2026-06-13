use std::{
  collections::{HashMap, HashSet},
  env,
  path::PathBuf,
};

use libp2p::{
  autonat,
  futures::StreamExt,
  gossipsub::{self, IdentTopic},
  identify, kad, mdns,
  multiaddr::Protocol,
  relay, request_response,
  swarm::SwarmEvent,
};
use tokio::{
  io::{AsyncBufReadExt, BufReader},
  select,
};

use crate::{
  file_protocol::{FilePartialMessage, FileWriteOutcome, hex_id},
  network::{self, ChatBehaviorEvent, FILE_TOPIC, MessageResponse},
};

pub async fn run() -> anyhow::Result<()> {
  let file_topic = IdentTopic::new(FILE_TOPIC);
  let mdns_enabled = env::var("CHAT_MDNS_ENABLED")?.parse::<bool>()?;
  let bootstrap_peers = env::var("CHAT_BOOTSTRAP_PEERS").map(|peers| {
    peers
      .split(',')
      .map(|s| s.to_string())
      .collect::<Vec<String>>()
  });

  let mut swarm = network::build_swarm(mdns_enabled)?;

  if let Ok(bootstrap_peers) = bootstrap_peers {
    network::add_bootstrap_peers(&mut swarm, bootstrap_peers)?;
  }

  println!("Peer ID: {:?}", swarm.local_peer_id());
  print_commands();

  let file_topic_hash = file_topic.hash();
  swarm
    .behaviour_mut()
    .gossipsub
    .enable_partials_for_topic(file_topic_hash.clone(), true);
  swarm.behaviour_mut().gossipsub.subscribe(&file_topic)?;

  let mut stdin = BufReader::new(tokio::io::stdin()).lines();
  let mut file_messages: HashMap<Vec<u8>, FilePartialMessage> = HashMap::new();
  let mut completed_files: HashSet<Vec<u8>> = HashSet::new();

  loop {
    select! {
        event = swarm.select_next_some() => {
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on: {}", address);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Connected to peer: {}", peer_id);
                }
                SwarmEvent::Behaviour(event) => match event {
                    ChatBehaviorEvent::Ping(_) => {}
                    ChatBehaviorEvent::Messaging(event) => match event {
                        request_response::Event::Message{message, peer, ..} => match message {
                            request_response::Message::Request {request, channel, ..} => {
                                println!("{peer} {:?}", request.message);
                                if let Err(err) = swarm.behaviour_mut().messaging.send_response(channel, MessageResponse { ack: true }) {
                                    println!("Error sending response: {:?}", err);
                                }
                            }
                            request_response::Message::Response {response, ..} => {
                                println!("{} Response ACK {response:?}", swarm.local_peer_id());
                            }
                        }
                        request_response::Event::ResponseSent{..} => {},
                        request_response::Event::OutboundFailure{peer, request_id, error, ..} => {
                            println!("Outbound failure: peer: {peer}, request_id: {request_id}, error: {error}");
                        },
                        request_response::Event::InboundFailure{peer, request_id, error, ..} => {
                            println!("Inbound failure: peer: {peer}, request_id: {request_id}, error: {error}");
                        },
                    }
                    ChatBehaviorEvent::Mdns(event) => match event {
                        mdns::Event::Discovered(new_peers) => {
                            for (peer_id, addr) in new_peers {
                                println!("Discovered peer: {}", peer_id);
                                swarm.add_peer_address(peer_id, addr.clone());
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        mdns::Event::Expired(_) => {
                        }
                    }
                    ChatBehaviorEvent::Identify(event) => match event {
                        identify::Event::Received{ peer_id, info, .. } => {
                            println!("New Identify received: {peer_id} - {info:?}");
                            let is_relay = info.protocols.iter().any(|protocol| {
                                *protocol == relay::HOP_PROTOCOL_NAME
                            });

                            for addr in info.listen_addrs {
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);

                                if is_relay {
                                    let listen_addr = addr.clone().with_p2p(peer_id).unwrap().with(Protocol::P2pCircuit);
                                    println!("Trying to listen on {:?}", listen_addr);
                                    swarm.listen_on(listen_addr).unwrap();
                                }
                            }
                        }
                        identify::Event::Sent { .. } => {},
                        identify::Event::Pushed { .. } =>{},
                        identify::Event::Error { .. } =>{},
                    }
                    ChatBehaviorEvent::Kademlia(event) => match event {
                        kad::Event::InboundRequest { .. } => {}
                        kad::Event::OutboundQueryProgressed {..} => {}
                        kad::Event::RoutingUpdated{
                            addresses,
                            peer,
                            ..
                        } => {
                            println!("New Routing updated! Peer: {peer} - {addresses:?}");
                            addresses.iter().for_each(|addr| {
                                if let Err(err) = swarm.dial(addr.clone()) {
                                    println!("Error dialing address {addr}: {err}");
                                }
                            });
                        }
                        kad::Event::UnroutablePeer{..} => {}
                        kad::Event::RoutablePeer{..} => {}
                        kad::Event::PendingRoutablePeer{..} => {}
                        kad::Event::ModeChanged{..} => {}
                    }
                    ChatBehaviorEvent::Autonat(event) => match event {
                        autonat::Event::InboundProbe(event) => {
                            println!("Inbound probe: {event:?}");
                        }
                        autonat::Event::OutboundProbe(event) => {
                            println!("Outbound probe: {event:?}");
                        }
                        autonat::Event::StatusChanged{new, old} => {
                            println!("Status changed: {old:?} -> {new:?}");
                        }
                    }
                    ChatBehaviorEvent::RelayServer(event) => {
                        println!("Relay server: {event:?}");
                    }
                    ChatBehaviorEvent::RelayClient(event) => {
                        println!("Relay client: {event:?}");
                    }
                    ChatBehaviorEvent::Dcutr(event) => {
                        println!("Dcutr: {:?}", event);
                    },
                    ChatBehaviorEvent::Gossipsub(event) => match event {
                        gossipsub::Event::Message {
                            propagation_source,
                            message_id,
                            message,
                        } => {
                            println!(
                                "Gossipsub full message from {propagation_source} with id {message_id}: {:?}",
                                String::from_utf8_lossy(&message.data),
                            );
                        }
                        gossipsub::Event::Partial {
                            topic_hash,
                            peer_id,
                            group_id,
                            message,
                            metadata,
                        } => {
                            if topic_hash != file_topic_hash {
                                println!("Gossipsub partial on non-file topic {topic_hash}: {group_id:?}");
                                continue;
                            }

                            let Some(metadata) = metadata else {
                                println!("Ignoring partial from {peer_id}: missing metadata");
                                continue;
                            };

                            let remote_metadata = match FilePartialMessage::parse_metadata(&metadata) {
                                Ok(parsed) => parsed,
                                Err(error) => {
                                    println!("Invalid partial metadata from {peer_id}: {error}");
                                    swarm.behaviour_mut().gossipsub.report_invalid_partial(peer_id, &topic_hash);
                                    continue;
                                }
                            };

                            let partial = file_messages
                                .entry(group_id.clone())
                                .or_insert_with(|| FilePartialMessage::empty(group_id.clone(), &remote_metadata));

                            if !partial.metadata_matches(&remote_metadata) {
                                println!(
                                    "Ignoring mismatched file metadata from {peer_id}: group={}",
                                    hex_id(&group_id)
                                );
                                swarm.behaviour_mut().gossipsub.report_invalid_partial(peer_id, &topic_hash);
                                continue;
                            }

                            let mut updated = false;
                            if let Some(message) = message {
                                match partial.merge_body(&message) {
                                    Ok(part_updated) => updated = part_updated,
                                    Err(error) => {
                                        println!("Invalid partial body from {peer_id}: {error}");
                                        swarm.behaviour_mut().gossipsub.report_invalid_partial(peer_id, &topic_hash);
                                        continue;
                                    }
                                }
                            }

                            let remote_has_useful_data =
                                partial.parts.iter().enumerate().any(|(index, part)| {
                                    part.is_none()
                                        && FilePartialMessage::has_part(
                                            &remote_metadata.bitmap,
                                            index,
                                        )
                                });

                            if updated || remote_has_useful_data {
                                if let Err(error) = swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .publish_partial(topic_hash.clone(), partial.clone())
                                {
                                    println!("Error republishing partial file update: {error:?}");
                                }
                            }

                            if partial.is_complete() {
                                if completed_files.insert(group_id.clone()) {
                                    match partial.write_to_disk().await {
                                        Ok(FileWriteOutcome::Written(path)) => {
                                            println!(
                                                "Completed file from {peer_id}: {} ({} bytes, {} parts) -> {}",
                                                partial.file_name,
                                                partial.file_size,
                                                partial.total_parts(),
                                                path.display()
                                            );
                                        }
                                        Ok(FileWriteOutcome::Duplicate(path)) => {
                                            println!(
                                                "Completed duplicate file from {peer_id}: {} ({} bytes, {} parts) -> {} already exists, skipped",
                                                partial.file_name,
                                                partial.file_size,
                                                partial.total_parts(),
                                                path.display()
                                            );
                                        }
                                        Err(error) => {
                                            println!(
                                                "Failed to write completed file {} from {peer_id}: {error}",
                                                partial.file_name
                                            );
                                        }
                                    }
                                } else {
                                    println!(
                                        "Received duplicate complete file update from {peer_id}: {}",
                                        partial.file_name
                                    );
                                }
                            } else {
                                println!(
                                    "Received file partial from {peer_id}: {} group={} parts={}/{}",
                                    partial.file_name,
                                    hex_id(&group_id),
                                    partial.parts.iter().filter(|part| part.is_some()).count(),
                                    partial.total_parts()
                                );
                            }
                        }
                        gossipsub::Event::Subscribed {
                            peer_id,
                            topic,
                            supports_partial,
                            requests_partial,
                        } => {
                            println!(
                                "Gossipsub subscribed: {peer_id} topic={topic} supports_partial={supports_partial} requests_partial={requests_partial}"
                            );
                            if topic == file_topic_hash && supports_partial {
                                let known_files =
                                    file_messages.values().cloned().collect::<Vec<_>>();
                                for file in known_files {
                                    if let Err(error) = swarm
                                        .behaviour_mut()
                                        .gossipsub
                                        .publish_partial(topic.clone(), file)
                                    {
                                        println!(
                                            "Error advertising known file to new subscriber {peer_id}: {error:?}"
                                        );
                                    }
                                }
                            }
                        }
                        gossipsub::Event::Unsubscribed { peer_id, topic } => {
                            println!("Gossipsub unsubscribed: {peer_id} topic={topic}");
                        }
                        gossipsub::Event::GossipsubNotSupported { peer_id } => {
                            println!("Gossipsub not supported by {peer_id}");
                        }
                        gossipsub::Event::SlowPeer {
                            peer_id,
                            failed_messages,
                        } => {
                            println!("Gossipsub slow peer: {peer_id} failed_messages={failed_messages:?}");
                        }
                    }
                }
                _ => {}
            }
        },
        Ok(Some(line)) = stdin.next_line() => {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line == "/help" {
                print_commands();
                continue;
            }

            if line == "/peers" {
                let peers = swarm
                    .connected_peers()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                println!("Connected peers: {}", peers.join(", "));
                continue;
            }

            let Some(path) = line.strip_prefix("/send ") else {
                println!("Unknown command. Use /send <path>, /peers, or /help.");
                continue;
            };

            let path = PathBuf::from(path.trim());
            let local_peer_id = *swarm.local_peer_id();
            let partial = match FilePartialMessage::from_path(&path, local_peer_id).await {
                Ok(partial) => partial,
                Err(error) => {
                    println!("Failed to read file {}: {error}", path.display());
                    continue;
                }
            };
            match swarm
                .behaviour_mut()
                .gossipsub
                .publish_partial(file_topic_hash.clone(), partial.clone())
            {
                Ok(()) => {
                    println!(
                        "Started file transfer: {} ({} bytes, {} parts, group={})",
                        partial.file_name,
                        partial.file_size,
                        partial.total_parts(),
                        hex_id(&partial.group_id)
                    );
                }
                Err(error) => {
                    println!("Error publishing partial file: {:?}", error);
                }
            }
            completed_files.insert(partial.group_id.clone());
            file_messages.insert(partial.group_id.clone(), partial);
        }
    }
  }
}

fn print_commands() {
  println!("Commands:");
  println!("  /send <path>  send a file through gossipsub partial messages");
  println!("  /peers        print connected peers");
  println!("  /help         print commands");
}
