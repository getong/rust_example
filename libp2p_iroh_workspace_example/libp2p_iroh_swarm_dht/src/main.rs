use std::{
  io::{self, BufRead, Write},
  str::FromStr,
  time::Duration,
};

use futures::StreamExt;
use libp2p::{
  Multiaddr, StreamProtocol, Transport,
  kad::{Event as KademliaEvent, store::MemoryStore},
  swarm::{NetworkBehaviour, Swarm, SwarmEvent},
};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  kademlia: libp2p::kad::Behaviour<MemoryStore>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("libp2p_iroh=warn,swarm_dht=warn")),
    )
    .init();

  let keypair = libp2p::identity::Keypair::generate_ed25519();
  let peer_id = keypair.public().to_peer_id();

  let transport = libp2p_iroh::Transport::new(Some(&keypair)).await?.boxed();

  println!("Local Peer ID: {peer_id}");

  let mut kad_config = libp2p::kad::Config::new(StreamProtocol::new("/example/kad/1.0.0"));
  kad_config.set_query_timeout(Duration::from_secs(60));

  let store = MemoryStore::new(peer_id);
  let mut kademlia = libp2p::kad::Behaviour::with_config(peer_id, store, kad_config);

  kademlia.set_mode(Some(libp2p::kad::Mode::Server));

  let behaviour = MyBehaviour { kademlia };

  let mut swarm = Swarm::new(
    transport,
    behaviour,
    peer_id,
    libp2p::swarm::Config::with_executor(Box::new(|fut| {
      tokio::spawn(fut);
    }))
    .with_idle_connection_timeout(Duration::from_secs(300)),
  );

  swarm.listen_on(Multiaddr::empty())?;

  println!("Swarm started. Enter commands:");
  println!("  <multiaddr>     - Dial a peer (/p2p/...)");
  println!("  put <key> <val> - Store a key-value pair in the DHT");
  println!("  get <key>       - Retrieve a value from the DHT");
  println!("  peers           - Display all known peers in routing table");
  println!();

  let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

  tokio::spawn(async move {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    loop {
      print!("> ");
      io::stdout().flush().ok();
      let mut line = String::new();
      if handle.read_line(&mut line).is_ok()
        && !line.is_empty()
        && tx.send(line.trim().to_string()).is_err()
      {
        break;
      }
    }
  });

  loop {
    tokio::select! {
        event = swarm.next() => {
            if let Some(event) = event {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on: {address}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad_event)) => {
                        match kad_event {
                            KademliaEvent::RoutingUpdated { peer, .. } => {
                                println!("Routing updated for peer: {peer}");
                            }
                            KademliaEvent::InboundRequest { request } => {
                                println!("Inbound request: {request:?}");
                            }
                            KademliaEvent::OutboundQueryProgressed { result, .. } => {
                                match result {
                                    libp2p::kad::QueryResult::GetRecord(Ok(libp2p::kad::GetRecordOk::FoundRecord(peer_record))) => {
                                        let key_str = String::from_utf8_lossy(peer_record.record.key.as_ref());
                                        let val_str = String::from_utf8_lossy(&peer_record.record.value);
                                        println!("Found record: {key_str} = {val_str}");
                                    }
                                    libp2p::kad::QueryResult::GetRecord(Err(e)) => {
                                        eprintln!("Get record failed: {e:?}");
                                    }
                                    libp2p::kad::QueryResult::PutRecord(Ok(libp2p::kad::PutRecordOk { key })) => {
                                        let key_str = String::from_utf8_lossy(key.as_ref());
                                        println!("Successfully stored key '{key_str}' in DHT");
                                    }
                                    libp2p::kad::QueryResult::PutRecord(Err(e)) => {
                                        eprintln!("Put record failed: {e:?}");
                                    }
                                    libp2p::kad::QueryResult::Bootstrap(Ok(libp2p::kad::BootstrapOk { peer, num_remaining })) => {
                                        println!("Bootstrap progress: {peer}, {num_remaining} remaining");
                                    }
                                    libp2p::kad::QueryResult::Bootstrap(Err(e)) => {
                                        eprintln!("Bootstrap failed: {e:?}");
                                    }
                                    _ => {
                                        println!("Query progressed: {result:?}");
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        println!("Connection established with {peer_id} at {endpoint:?}");
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, endpoint.get_remote_address().clone());
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        println!("Connection closed with {peer_id}: {cause:?}");
                    }
                    SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                        println!("Incoming connection from {send_back_addr}");
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        eprintln!("Outgoing connection error to {peer_id:?}: {error}");
                    }
                    SwarmEvent::IncomingConnectionError { send_back_addr, error, .. } => {
                        eprintln!("Incoming connection error from {send_back_addr}: {error}");
                    }
                    _ => {}
                }
            }
        }
        Some(cmd) = rx.recv() => {
            let parts: Vec<&str> = cmd.split_whitespace().collect();

            if cmd == "peers" {
                let mut peer_count = 0;
                println!("Known peers in routing table:");

                for kbucket in swarm.behaviour_mut().kademlia.kbuckets() {
                    for entry in kbucket.iter() {
                        peer_count += 1;
                        let peer_id = entry.node.key.preimage();
                        let status = entry.status;
                        println!("  {peer_id} (status: {status:?})");

                        // Show addresses for this peer
                        for addr in entry.node.value.iter() {
                            println!("    └─ {addr}");
                        }
                    }
                }

                if peer_count == 0 {
                    println!("  No peers in routing table");
                } else {
                    println!("Total peers: {peer_count}");
                }

                // Also show connected peers
                let connected: Vec<_> = swarm.connected_peers().collect();
                println!("\nCurrently connected peers: {}", connected.len());
                for peer in connected {
                    println!("  {peer}");
                }
            } else if parts.len() == 2 && parts[0] == "get" {
                let key = parts[1].as_bytes().to_vec();
                let key_str = String::from_utf8_lossy(&key);
                println!("Looking up key '{key_str}' in DHT...");
                swarm.behaviour_mut().kademlia.get_record(libp2p::kad::RecordKey::new(&key));
            } else if parts.len() == 3 && parts[0] == "put" {
                let key = parts[1].as_bytes().to_vec();
                let value = parts[2].as_bytes().to_vec();
                let record = libp2p::kad::Record::new(key.clone(), value);

                match swarm.behaviour_mut().kademlia.put_record(record, libp2p::kad::Quorum::One) {
                    Ok(_) => {
                        let key_str = String::from_utf8_lossy(&key);
                        println!("Storing key '{key_str}' in DHT");
                    }
                    Err(e) => {
                        eprintln!("Failed to put record: {e}");
                    }
                }
            } else if let Ok(addr) = Multiaddr::from_str(&cmd) {
                println!("Dialing: {addr}");
                if let Err(e) = swarm.dial(addr) {
                    eprintln!("Dial error: {e}");
                }
            } else {
                eprintln!("Unknown command: {cmd}");
                eprintln!("Available commands: put <key> <val>, get <key>, get_all, peers, <multiaddr>");
            }
        }
    }
  }
}
