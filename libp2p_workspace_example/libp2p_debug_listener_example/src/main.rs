mod network;

use std::{error::Error, io::Write, path::PathBuf};

use clap::Parser;
use futures::prelude::*;
use libp2p::{core::Multiaddr, multiaddr::Protocol};
use tokio::task::spawn;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let opt = Opt::parse();

  let (mut network_client, mut network_events, network_event_loop) =
    network::new(opt.secret_key_seed).await?;

  // Spawn the network task for it to run in the background.
  spawn(network_event_loop.run());

  // In case a listen address was provided use it, otherwise listen on any
  // address.
  match opt.listen_address {
    Some(addr) => network_client
      .start_listening(addr)
      .await
      .expect("Listening not to fail."),
    None => network_client
      .start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
      .await
      .expect("Listening not to fail."),
  };

  // In case the user provided an address of a peer on the CLI, dial it.
  if let Some(addr) = opt.peer {
    let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
      return Err("Expect peer multiaddr to contain peer ID.".into());
    };
    network_client
      .dial(peer_id, addr)
      .await
      .expect("Dial to succeed");
  }

  match opt.argument {
    // Providing a file.
    CliArgument::Provide { path, name } => {
      // Advertise oneself as a provider of the file on the DHT.
      network_client.start_providing(name.clone()).await;

      loop {
        match network_events.recv().await {
          // Reply with the content of the file on incoming requests.
          Some(network::Event::InboundRequest { request, channel }) => {
            if request == name {
              network_client
                .respond_file(std::fs::read(&path)?, channel)
                .await;
            }
          }
          // Handle mDNS peer discovery
          Some(network::Event::MdnsDiscovered { peer_id, addresses }) => {
            println!("Discovered peer via mDNS: {peer_id}");
            for addr in addresses {
              if let Err(e) = network_client.dial(peer_id, addr.clone()).await {
                println!("Failed to dial discovered peer {peer_id} at {addr}: {e}");
              }
            }
          }
          Some(network::Event::MdnsExpired { peer_id }) => {
            println!("mDNS peer expired: {peer_id}");
          }
          None => break,
        }
      }
    }
    // Locating and getting a file.
    CliArgument::Get { name } => {
      // Locate all nodes providing the file.
      let providers = network_client.get_providers(name.clone()).await;
      if providers.is_empty() {
        return Err(format!("Could not find provider for file {name}.").into());
      }

      // Request the content of the file from each node.
      let requests = providers.into_iter().map(|p| {
        let mut network_client = network_client.clone();
        let name = name.clone();
        async move { network_client.request_file(p, name).await }.boxed()
      });

      // Await the requests, ignore the remaining once a single one succeeds.
      let file_content = futures::future::select_ok(requests)
        .await
        .map_err(|_| "None of the providers returned file.")?
        .0;

      std::io::stdout().write_all(&file_content)?;
    }
  }

  Ok(())
}

#[derive(Parser, Debug)]
#[clap(name = "libp2p file sharing example")]
struct Opt {
  /// Fixed value to generate deterministic peer ID.
  #[clap(long)]
  secret_key_seed: Option<u8>,

  #[clap(long)]
  peer: Option<Multiaddr>,

  #[clap(long)]
  listen_address: Option<Multiaddr>,

  #[clap(subcommand)]
  argument: CliArgument,
}

#[derive(Debug, Parser)]
enum CliArgument {
  Provide {
    #[clap(long)]
    path: PathBuf,
    #[clap(long)]
    name: String,
  },
  Get {
    #[clap(long)]
    name: String,
  },
}
