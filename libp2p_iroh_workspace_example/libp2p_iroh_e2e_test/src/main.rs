use std::{env, time::Duration};

use futures::StreamExt;
use libp2p::{
  Multiaddr, StreamProtocol,
  kad::{Event as KademliaEvent, store::MemoryStore},
  swarm::{NetworkBehaviour, Swarm, SwarmEvent},
};
use libp2p_iroh::{Transport, TransportTrait};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  kademlia: libp2p::kad::Behaviour<MemoryStore>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let node_id = env::var("NODE_ID").unwrap_or_else(|_| "0".to_string());
  let bootstrap_peer = env::var("BOOTSTRAP_PEER").ok();
  let test_key = env::var("TEST_KEY").unwrap_or_else(|_| "testkey".to_string());
  let test_value = env::var("TEST_VALUE").unwrap_or_else(|_| "testvalue".to_string());
  let operation = env::var("OPERATION").unwrap_or_else(|_| "listen".to_string());

  let keypair = libp2p::identity::Keypair::generate_ed25519();
  let peer_id = keypair.public().to_peer_id();

  let transport = Transport::new(Some(&keypair)).await?.boxed();

  println!("NODE_{node_id}_PEER_ID={peer_id}");

  let mut kad_config = libp2p::kad::Config::new(StreamProtocol::new("/e2e-test/kad/1.0.0"));
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

  let mut listen_addr_printed = false;
  let mut connected_to_bootstrap = false;
  let mut operation_completed = false;

  // Timeout for the entire operation
  let timeout = tokio::time::sleep(Duration::from_secs(300));
  tokio::pin!(timeout);

  loop {
    tokio::select! {
        _ = &mut timeout => {
            eprintln!("NODE_{node_id}: Timeout reached");
            return Err("Operation timeout".into());
        }
        event = swarm.next() => {
            if let Some(event) = event {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        if !listen_addr_printed {
                            println!("NODE_{node_id}_LISTEN_ADDR={address}");
                            listen_addr_printed = true;

                            tokio::time::sleep(Duration::from_millis(500)).await;

                            if let Some(ref bootstrap) = bootstrap_peer {
                                if let Ok(addr) = bootstrap.parse::<Multiaddr>() {
                                    println!("NODE_{node_id}: Dialing bootstrap peer: {addr}");
                                    match swarm.dial(addr.clone()) {
                                        Ok(_) => println!("NODE_{node_id}: Dial initiated successfully"),
                                        Err(e) => eprintln!("NODE_{node_id}: Failed to dial: {e}"),
                                    }
                                } else {
                                    eprintln!("NODE_{node_id}: Failed to parse bootstrap address");
                                }
                            }
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id: connected_peer, .. } => {
                        println!("NODE_{node_id}: Connected to {connected_peer}");
                        swarm.behaviour_mut().kademlia.add_address(
                            &connected_peer,
                            format!("/p2p/{connected_peer}").parse().unwrap()
                        );

                        if bootstrap_peer.is_some() && !connected_to_bootstrap {
                            connected_to_bootstrap = true;

                            match operation.as_str() {
                                "put" => {
                                    println!("NODE_{node_id}: Storing key '{test_key}' = '{test_value}'");
                                    let record = libp2p::kad::Record::new(
                                        test_key.as_bytes().to_vec(),
                                        test_value.as_bytes().to_vec()
                                    );
                                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(
                                        record,
                                        libp2p::kad::Quorum::N(std::num::NonZeroUsize::new(2).unwrap())
                                    ) {
                                        eprintln!("NODE_{node_id}: Put failed: {e}");
                                    } else {
                                        println!("NODE_{node_id}: Put initiated (waiting for 2-node quorum)");
                                    }
                                }
                                "get" => {
                                    println!("NODE_{node_id}: Getting key '{test_key}'");
                                    swarm.behaviour_mut().kademlia.get_record(
                                        libp2p::kad::RecordKey::new(&test_key.as_bytes().to_vec())
                                    );
                                }
                                _ => {
                                    println!("NODE_{node_id}: Listening mode, no operation");
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad_event)) => {
                        match kad_event {
                            KademliaEvent::RoutingUpdated { peer, .. } => {
                                println!("NODE_{node_id}: Routing updated for peer: {peer}");
                            }
                            KademliaEvent::InboundRequest { request } => {
                                println!("NODE_{node_id}: Inbound request: {request:?}");
                            }
                            KademliaEvent::OutboundQueryProgressed { result, .. } => {
                                match result {
                                    libp2p::kad::QueryResult::GetRecord(Ok(
                                        libp2p::kad::GetRecordOk::FoundRecord(peer_record)
                                    )) => {
                                        let key_str = String::from_utf8_lossy(peer_record.record.key.as_ref());
                                        let val_str = String::from_utf8_lossy(&peer_record.record.value);
                                        println!("NODE_{node_id}_FOUND_RECORD: {key_str} = {val_str}");
                                        operation_completed = true;
                                    }
                                    libp2p::kad::QueryResult::GetRecord(Err(e)) => {
                                        eprintln!("NODE_{node_id}_GET_FAILED: {e:?}");
                                        operation_completed = true;
                                    }
                                    libp2p::kad::QueryResult::PutRecord(Ok(
                                        libp2p::kad::PutRecordOk { key }
                                    )) => {
                                        let key_str = String::from_utf8_lossy(key.as_ref());
                                        println!("NODE_{node_id}_PUT_SUCCESS: {key_str}");
                                        operation_completed = true;
                                    }
                                    libp2p::kad::QueryResult::PutRecord(Err(e)) => {
                                        eprintln!("NODE_{node_id}_PUT_FAILED: {e:?}");
                                        operation_completed = true;
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id: closed_peer, cause, .. } => {
                        println!("NODE_{node_id}: Connection closed with {closed_peer}: {cause:?}");
                    }
                    SwarmEvent::IncomingConnection { .. } => {
                        println!("NODE_{node_id}: Incoming connection attempt");
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        eprintln!("NODE_{node_id}: Outgoing connection error to {peer_id:?}: {error}");
                    }
                    SwarmEvent::IncomingConnectionError { error, .. } => {
                        eprintln!("NODE_{node_id}: Incoming connection error: {error}");
                    }
                    SwarmEvent::Dialing { peer_id, .. } => {
                        println!("NODE_{node_id}: Dialing {peer_id:?}...");
                    }
                    _ => {}
                }

                if operation_completed && operation != "listen" {
                    println!("NODE_{node_id}: Operation completed, staying alive for DHT replication");
                    operation_completed = false;
                }
            }
        }
    }
  }
}
