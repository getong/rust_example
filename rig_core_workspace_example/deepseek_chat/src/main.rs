use std::{
  env,
  io::{self, Write},
};

use anyhow::{Context, Result, bail};
use rig::{
  client::{CompletionClient, ProviderClient},
  completion::Chat,
  message::Message,
  providers::deepseek,
};

const DEFAULT_SYSTEM_PROMPT: &str =
  "You are a helpful assistant. Answer clearly, directly, and concisely.";
const PLACEHOLDER_API_KEY: &str = "your_deepseek_api_key_here";

#[tokio::main]
async fn main() -> Result<()> {
  let _ = dotenvy::dotenv();

  validate_api_key()?;

  let model = deepseek_model();
  let client =
    deepseek::Client::from_env().context("failed to create DeepSeek client from environment")?;
  let agent = client.agent(&model).preamble(DEFAULT_SYSTEM_PROMPT).build();

  run_chat(agent, &model).await
}

async fn run_chat<M>(agent: rig::agent::Agent<M>, model: &str) -> Result<()>
where
  M: rig::completion::CompletionModel + 'static,
{
  let stdin = io::stdin();
  let mut stdout = io::stdout();
  let mut history = Vec::new();

  println!("DeepSeek chat is ready.");
  println!("Model: {model}");
  println!("Type a message and press Enter. Type `exit` or `quit` to leave.");
  println!();

  loop {
    print!("you> ");
    stdout.flush().context("failed to flush stdout")?;

    let mut input = String::new();
    let bytes_read = stdin
      .read_line(&mut input)
      .context("failed to read input from stdin")?;

    if bytes_read == 0 {
      println!();
      break;
    }

    let input = input.trim();
    if input.is_empty() {
      continue;
    }

    if matches!(input, "exit" | "quit") {
      break;
    }

    match agent.chat(input, &history).await {
      Ok(response) => {
        println!("assistant> {response}");
        println!();

        history.push(Message::user(input));
        history.push(Message::assistant(response));
      }
      Err(error) => {
        eprintln!("request failed: {error}");
        eprintln!();
      }
    }
  }

  Ok(())
}

fn validate_api_key() -> Result<()> {
  let api_key = env::var("DEEPSEEK_API_KEY")
    .context("DEEPSEEK_API_KEY is missing. Add it to .env in the current directory.")?;
  let api_key = api_key.trim();

  if api_key.is_empty() || api_key == PLACEHOLDER_API_KEY {
    bail!("DEEPSEEK_API_KEY is empty or still using the placeholder value in .env");
  }

  Ok(())
}

fn deepseek_model() -> String {
  match env::var("DEEPSEEK_MODEL") {
    Ok(model) if !model.trim().is_empty() => model,
    _ => deepseek::DEEPSEEK_V4_FLASH.to_string(),
  }
}
