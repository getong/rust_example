use std::path::PathBuf;

use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use futures_lite::StreamExt;
use iroh::{endpoint::presets, protocol::Router, Endpoint, RelayMode, SecretKey};
use iroh_blobs::{
  api::{blobs::AddPathOptions, proto::ImportMode, remote::GetProgressItem},
  store::mem::MemStore,
  ticket::BlobTicket,
  BlobFormat, BlobsProtocol, ALPN,
};
use tracing_subscriber::{prelude::*, EnvFilter};

pub fn setup_logging() {
  tracing_subscriber::registry()
    .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
    .with(EnvFilter::from_default_env())
    .try_init()
    .ok();
}

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
  #[clap(subcommand)]
  command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
  Accept {
    path: PathBuf,
  },
  Connect {
    ticket: String,
    #[clap(long, short)]
    out: Option<PathBuf>,
  },
}

#[tokio::main]
async fn main() -> Result<()> {
  setup_logging();
  let cli = Cli::parse();

  let ep = Endpoint::builder(presets::Minimal)
    .secret_key(SecretKey::generate())
    .relay_mode(RelayMode::Disabled)
    .bind()
    .await?;

  let store = MemStore::new();

  match cli.command {
    Commands::Accept { path } => {
      if !path.is_file() {
        println!("Content must be a file.");
        ep.close().await;
        return Ok(());
      }
      let absolute = path.canonicalize()?;
      println!("Adding {} as {}...", path.display(), absolute.display());

      let blobs = BlobsProtocol::new(store.as_ref(), None);
      let router = Router::builder(ep.clone()).accept(ALPN, blobs).spawn();

      let tt = store
        .add_path_with_opts(AddPathOptions {
          path: absolute,
          format: BlobFormat::Raw,
          mode: ImportMode::Copy,
        })
        .await?;
      let ticket = BlobTicket::new(ep.addr(), tt.hash, tt.format);
      println!("{}", ticket);
      println!("To fetch the blob:\n\tcargo run -- connect \"{ticket}\" -o [FILE_PATH]");

      tokio::signal::ctrl_c().await?;
      router.shutdown().await?;
      ep.close().await;
      std::process::exit(0);
    }
    Commands::Connect { ticket, out } => {
      let ticket: BlobTicket = ticket.parse()?;
      let hash = ticket.hash();
      println!("Connecting to {:?}", ticket.addr());

      let conn = ep.connect(ticket.addr().clone(), ALPN).await?;
      println!("Connected, downloading {}...", hash);

      let mut progress = store
        .remote()
        .fetch(conn, ticket.hash_and_format())
        .stream();
      while let Some(item) = progress.next().await {
        match item {
          GetProgressItem::Progress(offset) => {
            println!("Downloaded {} bytes", offset);
          }
          GetProgressItem::Done(stats) => {
            println!(
              "Transferred {} bytes in {:?}",
              stats.total_bytes_read(),
              stats.elapsed
            );
          }
          GetProgressItem::Error(e) => {
            anyhow::bail!("download failed: {}", e);
          }
        }
      }

      if let Some(path) = out {
        let absolute = std::env::current_dir()?.join(&path);
        ensure!(!absolute.is_dir(), "output must not be a directory");
        store.export(hash, absolute).await?;
        println!("Exported to {}", path.display());
      }

      ep.close().await;
    }
  }

  Ok(())
}
