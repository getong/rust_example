mod demos;

use std::{process::ExitCode, time::Duration};

#[derive(Default)]
struct Options {
  allocate: bool,
  counters_and_lists: bool,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> ExitCode {
  let mut options = Options::default();
  for argument in std::env::args().skip(1) {
    match argument.as_str() {
      "--allocate" => options.allocate = true,
      "--counters-and-lists" => options.counters_and_lists = true,
      "--help" | "-h" => {
        println!("Usage: agones_sdk_example [--allocate] [--counters-and-lists]");
        println!("  --allocate            Mark this GameServer as Allocated after reservation");
        println!("  --counters-and-lists  Exercise Beta APIs using demo-rooms and demo-players");
        println!(
          "SDK endpoint: AGONES_SDK_GRPC_HOST (http://localhost), AGONES_SDK_GRPC_PORT (9357)"
        );
        return ExitCode::SUCCESS;
      }
      _ => {
        eprintln!("Unknown argument: {argument}. Use --help for usage.");
        return ExitCode::FAILURE;
      }
    }
  }

  println!("Rust Game Server has started!");

  match run(options).await {
    Ok(_) => {
      println!("Rust Game Server finished.");
      ExitCode::SUCCESS
    }
    Err(msg) => {
      eprintln!("{}", msg);
      ExitCode::FAILURE
    }
  }
}

async fn run(options: Options) -> Result<(), String> {
  println!("Creating SDK instance");
  let mut sdk = agones::Sdk::new_with_host(None, None, None)
    .await
    .map_err(|e| format!("unable to create sdk client: {}", e))?;

  // Spawn a task that will send health checks every 2 seconds. If this current
  // thread/task panics or dropped, the health check will also be stopped
  let _health = {
    let health_tx = sdk.health_check();
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    tokio::task::spawn(async move {
      let mut interval = tokio::time::interval(Duration::from_secs(2));

      loop {
        tokio::select! {
          _ = interval.tick() => {
            if health_tx
              .send(())
              .await.is_err() {
                eprintln!("Health check receiver was dropped");
                break;
              }
          }
          _ = &mut rx => {
            println!("Health check task canceled");
            break;
          }
        }
      }
    });

    tx
  };

  let _watch = {
    let mut watch_client = sdk.clone();
    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

    tokio::task::spawn(async move {
      println!("Starting to watch GameServer updates...");
      match watch_client.watch_gameserver().await {
        Err(e) => println!("Failed to watch for GameServer updates: {}", e),
        Ok(mut stream) => loop {
          tokio::select! {
            gs = stream.message() => {
              match gs {
                Ok(Some(gs)) => {
                  print_gameserver(&gs);
                }
                Ok(None) => {
                  println!("Server closed the GameServer watch stream");
                  break;
                }
                Err(e) => {
                  eprintln!("GameServer Update stream encountered an error: {}", e);
                  break;
                }
              }

            }
            _ = &mut rx => {
              println!("Shutting down GameServer watch loop");
              break;
            }
          }
        },
      }
    });

    tx
  };

  println!("Setting a label");
  sdk
    .set_label("test-label", "test-value")
    .await
    .map_err(|e| format!("Could not run SetLabel(): {}. Exiting!", e))?;

  println!("Setting an annotation");
  sdk
    .set_annotation("test-annotation", "test value")
    .await
    .map_err(|e| format!("Could not run SetAnnotation(): {}. Exiting!", e))?;

  println!("Marking server as ready...");
  sdk
    .ready()
    .await
    .map_err(|e| format!("Could not run Ready(): {}. Exiting!", e))?;

  println!("...marked Ready");

  println!("Setting as Reserved for 5 seconds");
  sdk
    .reserve(Duration::from_secs(5))
    .await
    .map_err(|e| format!("Could not run Reserve(): {}. Exiting!", e))?;
  println!("...Reserved");

  tokio::time::sleep(Duration::from_secs(6)).await;

  println!("Getting GameServer details...");
  let gameserver = sdk
    .get_gameserver()
    .await
    .map_err(|e| format!("Could not run GameServer(): {}. Exiting!", e))?;

  print_gameserver(&gameserver);

  if options.allocate {
    println!("Allocating this GameServer...");
    sdk
      .allocate()
      .await
      .map_err(|e| format!("Could not run Allocate(): {e}"))?;
    println!("...marked for Allocation");
  }

  if options.counters_and_lists {
    demos::counters_and_lists(sdk.beta().clone())
      .await
      .map_err(|e| format!("Counters and Lists demo failed: {e}"))?;
  }

  for i in 0 .. 10 {
    let time = i * 10;
    println!("Running for {} seconds", time);

    tokio::time::sleep(Duration::from_secs(10)).await;

    if i == 5 {
      println!("Shutting down after 60 seconds...");
      sdk
        .shutdown()
        .await
        .map_err(|e| format!("Could not run Shutdown: {}. Exiting!", e))?;
      println!("...marked for Shutdown");
      break;
    }
  }

  Ok(())
}

fn print_gameserver(gameserver: &agones::GameServer) {
  if let Some(metadata) = &gameserver.object_meta {
    println!(
      "GameServer name: {}, namespace: {}, uid: {}",
      metadata.name, metadata.namespace, metadata.uid
    );
    println!("GameServer labels: {:?}", metadata.labels);
    println!("GameServer annotations: {:?}", metadata.annotations);
  } else {
    eprintln!("GameServer metadata is missing");
  }

  if let Some(status) = &gameserver.status {
    println!(
      "GameServer state: {}, address: {}",
      status.state, status.address
    );
    for port in &status.ports {
      println!("GameServer port: {} = {}", port.name, port.port);
    }
  } else {
    eprintln!("GameServer status is missing");
  }
}
