use opencode_rs::{
  ClientBuilder,
  server::{ManagedServer, ServerOptions},
  types::{
    event::Event,
    message::{Part, PromptPart, PromptRequest},
    session::CreateSessionRequest,
  },
};
use tokio::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Initialize tracing for debug output
  tracing_subscriber::fmt::init();

  let current_dir = std::env::current_dir()?;

  // Start a managed OpenCode server instead of assuming one is already listening
  let server = ManagedServer::start(
    ServerOptions::new()
      .hostname("127.0.0.1")
      .startup_timeout_ms(10_000)
      .directory(&current_dir),
  )
  .await?;
  println!("Started OpenCode server at {}", server.url());

  // Build client against the managed server
  let client = ClientBuilder::new()
    .base_url(server.url().to_string())
    .directory(current_dir.to_string_lossy())
    .build()?;

  // Create session
  let session = client
    .sessions()
    .create(&CreateSessionRequest::default())
    .await?;
  println!("Created session: {}", session.id);

  // Subscribe to session events BEFORE sending prompt
  let mut subscription = client.subscribe_session(&session.id)?;
  println!("Subscribed to events");

  // Send prompt
  client
    .messages()
    .prompt_async(
      &session.id,
      &PromptRequest {
        parts: vec![PromptPart::Text {
          text: "Write a haiku about Rust programming".into(),
          synthetic: None,
          ignored: None,
          metadata: None,
        }],
        message_id: None,
        model: None,
        agent: None,
        no_reply: None,
        system: None,
        variant: None,
      },
    )
    .await?;
  println!("Prompt sent, streaming events...\n");

  // Stream events for a bounded amount of time because some opencode versions
  // close /event aggressively and rely on reconnection.
  let deadline = Instant::now() + Duration::from_secs(20);
  let mut session_finished = false;

  while Instant::now() < deadline {
    match tokio::time::timeout(Duration::from_millis(500), subscription.recv()).await {
      Ok(Some(Event::SessionIdle { .. })) => {
        println!("\n[Session completed]");
        session_finished = true;
        break;
      }
      Ok(Some(Event::SessionError { properties })) => {
        eprintln!("\n[Session error: {:?}]", properties.error);
        break;
      }
      Ok(Some(Event::MessagePartUpdated { properties })) => {
        if let Some(delta) = &properties.delta {
          print!("{}", delta);
        }
      }
      Ok(Some(Event::ServerHeartbeat { .. })) => {
        // Heartbeat received, connection alive
      }
      Ok(Some(_)) => {}
      Ok(None) => {}
      Err(_) => {}
    }
  }

  if !session_finished {
    println!("\n[Streaming timed out, falling back to session history]");
  }

  let messages = client.messages().list(&session.id).await?;
  if let Some(assistant) = messages.iter().rev().find(|message| message.role() == "assistant") {
    let text = assistant
      .parts
      .iter()
      .filter_map(|part| match part {
        Part::Text { text, .. } | Part::Reasoning { text, .. } => Some(text.as_str()),
        _ => None,
      })
      .collect::<Vec<_>>()
      .join("");

    if !text.trim().is_empty() {
      println!("\nAssistant reply:\n{}", text.trim());
    } else {
      println!("\nAssistant message was created, but it did not contain text parts.");
    }
  }

  // Cleanup
  client.sessions().delete(&session.id).await?;
  println!("Session deleted");

  server.stop().await?;
  println!("Server stopped");

  Ok(())
}
