mod stream_chat;

use std::{env, process::ExitCode};

use stream_chat::{
  StreamChatCall, StreamChatCallImpl, StreamChatExpiringTokenRequest, StreamChatTokenRequest,
};

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
  let expiration_seconds = args
    .next()
    .or_else(|| env::var("STREAM_TOKEN_EXPIRATION_SECS").ok())
    .map(|value| {
      value
        .parse::<u64>()
        .map_err(|err| format!("invalid expiration seconds `{value}`: {err}"))
    })
    .transpose()?;

  let response = if let Some(expiration_seconds) = expiration_seconds {
    StreamChatCallImpl::create_token_with_expiration(&StreamChatExpiringTokenRequest {
      api_key,
      api_secret,
      user_id,
      expiration_seconds,
    })
  } else {
    StreamChatCallImpl::create_token(&StreamChatTokenRequest {
      api_key,
      api_secret,
      user_id,
    })
  };

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
  "usage: streamchat_go_example <api-key> <api-secret> [user-id] [expiration-seconds]\nor set \
   STREAM_API_KEY / STREAM_API_SECRET / STREAM_USER_ID / STREAM_TOKEN_EXPIRATION_SECS"
    .to_string()
}
