mod stream_chat;

use std::{env, process::ExitCode};

use stream_chat::{StreamChatCall, StreamChatCallImpl, StreamChatTokenRequest};

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      eprintln!("{err}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), String> {
  let mut args = env::args().skip(1);

  let api_key = args
    .next()
    .or_else(|| env::var("STREAM_API_KEY").ok())
    .ok_or_else(usage)?;
  let api_secret = args
    .next()
    .or_else(|| env::var("STREAM_API_SECRET").ok())
    .ok_or_else(usage)?;
  let user_id = args
    .next()
    .or_else(|| env::var("STREAM_USER_ID").ok())
    .unwrap_or_else(|| "john".to_string());

  let response = StreamChatCallImpl::create_token(&StreamChatTokenRequest {
    api_key,
    api_secret,
    user_id,
  });

  if response.error.is_empty() {
    println!("{}", response.token);
    Ok(())
  } else {
    Err(format!(
      "failed to create Stream Chat token: {}",
      response.error
    ))
  }
}

fn usage() -> String {
  "usage: streamchat_go_example <api-key> <api-secret> [user-id]\nor set STREAM_API_KEY / \
   STREAM_API_SECRET / STREAM_USER_ID"
    .to_string()
}
