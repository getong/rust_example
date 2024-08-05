use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::{env, process};

use memmap2::MmapOptions;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize)]
struct Entry {
  url: String,
}

#[derive(Deserialize)]
struct Tab {
  entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Window {
  tabs: Vec<Tab>,
}

#[derive(Deserialize)]
struct SessionStore {
  windows: Vec<Window>,
}

fn main() -> anyhow::Result<()> {
  let mut args = env::args_os().collect::<Vec<_>>();
  if args.len() != 2 {
    eprintln!("Usage: {} <profile>", args[0].to_string_lossy());
    process::exit(1);
  }

  let mut path = PathBuf::from(args.remove(1));
  path.push("sessionstore-backups/recovery.jsonlz4");

  let file = File::open(&path)?;
  let mmap = unsafe { MmapOptions::new().map(&file)? };
  let buf = lz4_flex::decompress_size_prepended(&mmap[8 ..])?;

  let session = serde_json::from_slice::<SessionStore>(&buf)?;
  let mut domains = HashMap::<_, u32>::new();
  for window in session.windows {
    for tab in window.tabs {
      if let Some(entry) = tab.entries.last() {
        let url = Url::parse(&entry.url)?;
        // println!("{url}"); // uncomment this to show all URLs
        if let Some(host) = url.host_str() {
          *domains.entry(host.to_string()).or_default() += 1;
        }
      }
    }
  }

  let mut domains = domains.into_iter().collect::<Vec<_>>();
  domains.sort_unstable_by_key(|p| Reverse(p.1));
  for (domain, count) in domains.into_iter().take(10) {
    println!("{} {}", domain, count);
  }

  Ok(())
}
