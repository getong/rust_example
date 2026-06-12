use std::{
  env,
  io::{self, Write},
  time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use rig_core::{client::CompletionClient, completion::Chat, providers::deepseek};

const DEFAULT_SYSTEM_PROMPT: &str =
  "You are a helpful assistant. Answer clearly, directly, and concisely.";
const PLACEHOLDER_API_KEY: &str = "your_deepseek_api_key_here";
const DEFAULT_TIMEOUT_SECS: u64 = 60;

#[tokio::main]
async fn main() -> Result<()> {
  let _ = dotenvy::dotenv();

  validate_api_key()?;

  let model = deepseek_model();
  let timeout = request_timeout();
  let client = build_client(timeout).with_context(|| {
    format!(
      "failed to create DeepSeek client with timeout {}s",
      timeout.as_secs()
    )
  })?;
  let agent = client.agent(&model).preamble(DEFAULT_SYSTEM_PROMPT).build();

  run_chat(agent, &model, timeout).await
}

async fn run_chat<M>(agent: rig_core::agent::Agent<M>, model: &str, timeout: Duration) -> Result<()>
where
  M: rig_core::completion::CompletionModel + 'static,
{
  let stdin = io::stdin();
  let mut stdout = io::stdout();
  let mut history = Vec::new();

  println!("DeepSeek chat is ready.");
  println!("Model: {model}");
  println!("Base URL: https://api.deepseek.com");
  println!("Request timeout: {}s", timeout.as_secs());
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

    eprintln!(
      "[debug] sending request to DeepSeek model `{model}` (history messages: {})",
      history.len()
    );
    let started_at = Instant::now();

    match agent.chat(input, &mut history).await {
      Ok(response) => {
        eprintln!("[debug] request completed in {:.2?}", started_at.elapsed());
        println!("assistant> {response}");
        println!();
      }
      Err(error) => {
        eprintln!("[debug] request failed after {:.2?}", started_at.elapsed());
        print_error_chain(&error);
        eprintln!(
          "hint: check network reachability, API key validity, model name, and timeout setting."
        );
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

fn request_timeout() -> Duration {
  let secs = env::var("DEEPSEEK_TIMEOUT_SECS")
    .ok()
    .and_then(|value| value.trim().parse::<u64>().ok())
    .filter(|secs| *secs > 0)
    .unwrap_or(DEFAULT_TIMEOUT_SECS);

  Duration::from_secs(secs)
}

fn build_client(timeout: Duration) -> Result<deepseek::Client> {
  let api_key = env::var("DEEPSEEK_API_KEY")
    .context("DEEPSEEK_API_KEY is missing. Add it to .env in the current directory.")?;

  let http_client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .timeout(timeout)
    .build()
    .context("failed to build reqwest client")?;

  let mut headers = HeaderMap::new();
  headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

  deepseek::Client::builder()
    .api_key(api_key)
    .http_headers(headers)
    .http_client(http_client)
    .build()
    .map_err(Into::into)
}

fn print_error_chain(error: &dyn std::error::Error) {
  eprintln!("request failed: {error}");

  let mut index = 0usize;
  let mut current = error.source();
  while let Some(source) = current {
    index += 1;
    eprintln!("  caused by[{index}]: {source}");
    current = source.source();
  }
}
