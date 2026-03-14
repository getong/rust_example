use std::{env, error::Error, io};

use chainlink_data_streams_report::{
  feed_id::ID,
  report::{
    Report,
    compress::{CompressionError, compress_report, compress_report_raw},
    decode_full_report,
    v3::ReportDataV3,
  },
};
use chainlink_data_streams_sdk::{
  client::Client,
  config::{Config, InsecureSkipVerify, WebSocketHighAvailability},
  stream::Stream,
};
use tokio::time::{Duration, timeout};

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const DEFAULT_REST_URL: &str = "https://api.testnet-dataengine.chain.link";
const DEFAULT_WS_URL: &str = "wss://ws.testnet-dataengine.chain.link";
const DEFAULT_FEED_ID: &str = "0x000359843a543ee2fe414dc14c7e7920ef10f4372990b79d6361cdc0dd1ba782";
const SAMPLE_FULL_REPORT_HEX: &str = "0006bd87830d5f336e205cf5c63329a1dab8f5d56812eaeb7c69300e66ab8e22000000000000000000000000000000000000000000000000000000000cf7ed13000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000022000000000000000000000000000000000000000000000000000000000000003000101000101000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012000030ab7d02fbba9c6304f98824524407b1f494741174320cfd17a2c22eec1de0000000000000000000000000000000000000000000000000000000066a8f5c60000000000000000000000000000000000000000000000000000000066a8f5c60000000000000000000000000000000000000000000000000057810653dd9000000000000000000000000000000000000000000000000000541315da76d6100000000000000000000000000000000000000000000000000000000066aa474600000000000000000000000000000000000000000000000009a697ee4230350400000000000000000000000000000000000000000000000009a6506d1426d00000000000000000000000000000000000000000000000000009a77d03ae355fe0000000000000000000000000000000000000000000000000000000000000000672bac991f5233df89f581dc02a89dd8d48419e3558b247d3e65f4069fa45c36658a5a4820dc94fc47a88a21d83474c29ee38382c46b6f9a575b9ce8be4e689c03c76fac19fbec4a29dba704c72cc003a6be1f96af115e322321f0688e24720a5d9bd7136a1d96842ec89133058b888b2e6572b5d4114de2426195e038f1c9a5ce50016b6f5a5de07e08529b845e1c622dcbefa0cfa2ffd128e9932ecee8efd869bc56d09a50ceb360a8d366cfa8eefe3f64279c88bdbc887560efa9944238eb000000000000000000000000000000000000000000000000000000000000000060e2a800f169f26164533c7faff6c9073cd6db240d89444d3487113232f9c31422a0993bb47d56807d0dc26728e4c8424bb9db77511001904353f1022168723010c46627c890be6e701e766679600696866c888ec80e7dbd428f5162a24f2d8262f846bdb06d9e46d295dd8e896fb232be80534b0041660fe4450a7ede9bc3b230722381773a4ae81241568867a759f53c2bdd05d32b209e78845fc58203949e50a608942b270c456001e578227ad00861cf5f47b27b09137a0c4b7f8b4746cef";

struct RuntimeOptions {
  feed_id: ID,
  ws_messages: usize,
  enable_ws: bool,
  config: Option<Config>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
  println!("== chainlink-data-streams-report ==");
  run_report_crate_demo()?;

  let options = read_runtime_options()?;

  match &options.config {
    Some(config) => {
      println!();
      println!("== chainlink-data-streams-sdk REST ==");
      if let Err(error) = run_rest_sdk_demo(config, options.feed_id).await {
        eprintln!("REST demo failed: {error}");
      }

      println!();
      println!("== chainlink-data-streams-sdk WebSocket ==");
      if options.enable_ws {
        if let Err(error) =
          run_websocket_sdk_demo(config, options.feed_id, options.ws_messages).await
        {
          eprintln!("WebSocket demo failed: {error}");
        }
      } else {
        println!("Skip WebSocket demo. Set CHAINLINK_ENABLE_WS=true to read live stream data.");
      }
    }
    None => {
      println!();
      println!("== chainlink-data-streams-sdk ==");
      println!("Skip network demos because credentials are not configured.");
      println!("Set CHAINLINK_API_KEY and CHAINLINK_API_SECRET to enable REST/WebSocket.");
      println!(
        "Optional envs: CHAINLINK_FEED_ID, CHAINLINK_REST_URL, CHAINLINK_WS_URL, \
         CHAINLINK_ENABLE_WS."
      );
    }
  }

  Ok(())
}

fn run_report_crate_demo() -> AppResult<()> {
  let full_report = decode_hex_payload(SAMPLE_FULL_REPORT_HEX)?;
  let (_report_context, report_blob) = decode_full_report(&full_report)?;
  let report_data = ReportDataV3::decode(&report_blob)?;
  let reencoded = report_data.abi_encode()?;

  let report = Report {
    feed_id: report_data.feed_id,
    valid_from_timestamp: report_data.valid_from_timestamp as usize,
    observations_timestamp: report_data.observations_timestamp as usize,
    full_report: format!("0x{SAMPLE_FULL_REPORT_HEX}"),
  };

  let compressed_raw = compress_report_raw(&full_report).map_err(map_compression_error)?;
  let compressed_report_json = compress_report(report.clone()).map_err(map_compression_error)?;

  print_report_summary("Built-in V3 sample", &report)?;
  println!("Decoded report_blob bytes: {}", report_blob.len());
  println!("ABI re-encoded bytes: {}", reencoded.len());
  println!(
    "Snappy compressed full report bytes: {}",
    compressed_raw.len()
  );
  println!(
    "Snappy compressed serialized Report bytes: {}",
    compressed_report_json.len()
  );

  Ok(())
}

async fn run_rest_sdk_demo(config: &Config, feed_id: ID) -> AppResult<()> {
  let client = Client::new(config.clone())?;
  let feeds = client.get_feeds().await?;

  println!("Configured feed: {feed_id}");
  println!("Available feeds from API: {}", feeds.len());
  if let Some(first_feed) = feeds.first() {
    println!(
      "First feed from API: {} (schema v{})",
      first_feed.feed_id,
      first_feed.version().0
    );
  }

  let latest = client.get_latest_report(feed_id).await?;
  print_report_summary("Latest report", &latest.report)?;

  let timestamp = latest.report.observations_timestamp as u128;
  let exact = client.get_report(feed_id, timestamp).await?;
  print_report_summary("Exact report by timestamp", &exact.report)?;

  let page = client
    .get_reports_page_with_limit(feed_id, latest.report.valid_from_timestamp as u128, 2)
    .await?;
  println!("Reports page with limit=2 returned {} item(s).", page.len());
  for (index, report) in page.iter().enumerate() {
    println!(
      "Page[{index}] observations_timestamp={}",
      report.observations_timestamp
    );
  }

  let bulk = client.get_reports_bulk(&[feed_id], timestamp).await?;
  println!("Bulk report query returned {} item(s).", bulk.len());

  Ok(())
}

async fn run_websocket_sdk_demo(config: &Config, feed_id: ID, ws_messages: usize) -> AppResult<()> {
  let mut stream = Stream::new(config, vec![feed_id]).await?;
  stream.listen().await?;

  for index in 0 .. ws_messages {
    let response = timeout(Duration::from_secs(15), stream.read()).await??;
    print_report_summary(
      &format!("WebSocket report #{}", index + 1),
      &response.report,
    )?;
  }

  stream.close().await?;

  let stats = stream.get_stats();
  println!(
    "Stream stats: accepted={}, deduplicated={}, total_received={}",
    stats.accepted, stats.deduplicated, stats.total_received
  );

  Ok(())
}

fn read_runtime_options() -> AppResult<RuntimeOptions> {
  let feed_id = env::var("CHAINLINK_FEED_ID")
    .ok()
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| DEFAULT_FEED_ID.to_string());
  let feed_id = ID::from_hex_str(&feed_id)?;

  let ws_messages = env::var("CHAINLINK_WS_MESSAGES")
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .unwrap_or(1)
    .max(1);
  let enable_ws = env_flag("CHAINLINK_ENABLE_WS");

  let api_key = env::var("CHAINLINK_API_KEY").ok();
  let api_secret = env::var("CHAINLINK_API_SECRET").ok();

  let config = match (api_key, api_secret) {
    (Some(api_key), Some(api_secret))
      if !api_key.trim().is_empty() && !api_secret.trim().is_empty() =>
    {
      let rest_url = env::var("CHAINLINK_REST_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_REST_URL.to_string());
      let ws_url = env::var("CHAINLINK_WS_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_WS_URL.to_string());

      let mut builder = Config::new(api_key, api_secret, rest_url, ws_url);

      if env_flag("CHAINLINK_WS_HA") {
        builder = builder.with_ws_ha(WebSocketHighAvailability::Enabled);
      }

      if let Ok(max_reconnect) = env::var("CHAINLINK_WS_MAX_RECONNECT")
        .unwrap_or_default()
        .parse::<usize>()
      {
        builder = builder.with_ws_max_reconnect(max_reconnect);
      }

      if env_flag("CHAINLINK_INSECURE_SKIP_VERIFY") {
        builder = builder.with_insecure_skip_verify(InsecureSkipVerify::Enabled);
      }

      Some(builder.build()?)
    }
    _ => None,
  };

  Ok(RuntimeOptions {
    feed_id,
    ws_messages,
    enable_ws,
    config,
  })
}

fn print_report_summary(label: &str, report: &Report) -> AppResult<()> {
  let full_report = decode_hex_payload(&report.full_report)?;
  let (_report_context, report_blob) = decode_full_report(&full_report)?;
  let report_data = ReportDataV3::decode(&report_blob)?;

  println!("{label}:");
  println!("  feed_id={}", report.feed_id);
  println!("  valid_from_timestamp={}", report.valid_from_timestamp);
  println!("  observations_timestamp={}", report.observations_timestamp);
  println!("  benchmark_price={}", report_data.benchmark_price);
  println!("  bid={}", report_data.bid);
  println!("  ask={}", report_data.ask);
  println!("  expires_at={}", report_data.expires_at);

  Ok(())
}

fn decode_hex_payload(payload: &str) -> AppResult<Vec<u8>> {
  let payload = payload
    .strip_prefix("0x")
    .or_else(|| payload.strip_prefix("0X"))
    .unwrap_or(payload);

  Ok(hex::decode(payload)?)
}

fn env_flag(name: &str) -> bool {
  match env::var(name) {
    Ok(value) => matches!(
      value.trim().to_ascii_lowercase().as_str(),
      "1" | "true" | "yes" | "on"
    ),
    Err(_) => false,
  }
}

fn map_compression_error(error: CompressionError) -> io::Error {
  io::Error::other(format!("compression error: {error:?}"))
}
