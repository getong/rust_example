use std::{error::Error, time::Duration};

use tokio::{
  signal::unix::{signal, SignalKind},
  time::sleep,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Create a signal stream for the INT and TERM signals
  let mut sig_int = signal(SignalKind::interrupt())?;
  let mut sig_term = signal(SignalKind::terminate())?;

  // Spawn a task to handle the application logic
  let app_task = tokio::spawn(async move {
    // Application logic goes here...
    // For demonstration purposes, we'll simply print a message repeatedly
    loop {
      println!("Application is running...");
      sleep(Duration::from_secs(1)).await;
    }
  });

  // Wait for the INT or TERM signals
  tokio::select! {
      _ = sig_int.recv() => {
          println!("Received INT signal...");
      }
      _ = sig_term.recv() => {
          println!("Received TERM signal...");
      }
  }

  // Perform any necessary cleanup or finalization here...
  // For demonstration purposes, we'll simply sleep for a while
  println!("Performing cleanup...");
  sleep(Duration::from_secs(3)).await;
  println!("Cleanup complete.");

  // Terminate the application gracefully
  println!("Application is shutting down...");

  // Cancel the application task
  app_task.abort();

  Ok(())
}
