use std::{
  env,
  io::{self, Write},
  time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use rig_core::{
  completion::{CompletionRequest, Message},
  providers::{deepseek, openai},
};
use rig_reqwest::ReqwestClient;

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
  let chat = client.chat(&model);

  run_chat(chat, &model, timeout).await
}

async fn run_chat(
  chat: rig_core::Model<openai::wire::Chat>,
  model: &str,
  timeout: Duration,
) -> Result<()> {
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

    let request = CompletionRequest::new(input)
      .preamble(DEFAULT_SYSTEM_PROMPT)
      .messages(history.iter().cloned());
    match chat.call(request).await {
      Ok(response) => {
        eprintln!("[debug] request completed in {:.2?}", started_at.elapsed());
        println!("assistant> {}", response.text());
        println!();
        if let Some(message) = response.message() {
          history.push(Message::user(input));
          history.push(message);
        }
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

fn build_client(timeout: Duration) -> Result<openai::OpenAI> {
  let api_key = env::var("DEEPSEEK_API_KEY")
    .context("DEEPSEEK_API_KEY is missing. Add it to .env in the current directory.")?;

  let http_client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .timeout(timeout)
    .build()
    .context("failed to build reqwest client")?;

  Ok(
    openai::OpenAIConfig::with_key(&openai::wire::DEEPSEEK, api_key)
      .connect(ReqwestClient::from(http_client)),
  )
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
