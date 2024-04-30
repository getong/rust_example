use notify::{RecursiveMode, Result, Watcher};
use std::path::Path;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> Result<()> {
  // Automatically select the best implementation for your platform.
  let (tx, mut rx) = watch::channel(());

  let mut watcher = notify::recommended_watcher(move |res| match res {
    Ok(_event) => {
      _ = tx.send(());
    }
    Err(e) => println!("watch error: {:?}", e),
  })?;

  // Add a path to be watched. All files and directories at that path and
  // below will be monitored for changes.
  watcher.watch(Path::new("."), RecursiveMode::Recursive)?;

  loop {
    _ = rx.changed().await;
    println!("file changed");
  }
}
