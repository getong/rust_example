use std::{
  borrow::Cow,
  cell::RefCell,
  rc::Rc,
  sync::{Arc, Mutex},
};

use deno_core::{ExtensionFileSource, OpState, Resource, ResourceId, error::AnyError, op2};
use deno_error::JsErrorBox;
use tokio::{
  sync::mpsc,
  time::{Duration, MissedTickBehavior},
};

const DUPLEX_API_SPECIFIER: &str = "ext:libmainworker_duplex_ext/duplex_api.ts";
const DUPLEX_API_SOURCE: &str = include_str!("duplex_api.ts");

/// Channel pair handed from main.rs into the deno_core extension.
/// Uses `serde_json::Value` instead of `String` to avoid an extra
/// serialize → deserialize round-trip on the Rust side (P0).
pub(crate) struct DuplexChannelPair {
  pub(crate) inbound_rx: mpsc::Receiver<serde_json::Value>,
  pub(crate) outbound_tx: mpsc::Sender<serde_json::Value>,
}

#[derive(Clone)]
struct DuplexChannelSlot {
  channels: Arc<Mutex<Option<DuplexChannelPair>>>,
}

/// The resource exposed to JS ops.
///
/// P2: `inbound_rx` uses a plain `RefCell` instead of `tokio::sync::Mutex`
/// because all deno_core ops run on the same single-threaded `LocalSet` —
/// there is never any cross-thread contention.
#[derive(Debug)]
struct TokioDuplexResource {
  inbound_rx: RefCell<mpsc::Receiver<serde_json::Value>>,
  outbound_tx: mpsc::Sender<serde_json::Value>,
}

impl Resource for TokioDuplexResource {
  fn name(&self) -> Cow<'_, str> {
    "mainworkerDuplex".into()
  }
}

#[op2(fast)]
#[smi]
fn op_duplex_open(state: &mut OpState) -> Result<ResourceId, JsErrorBox> {
  let slot = state.borrow::<DuplexChannelSlot>().clone();
  let mut guard = slot
    .channels
    .lock()
    .map_err(|_| JsErrorBox::generic("failed to lock duplex channel slot"))?;
  let channels = guard
    .take()
    .ok_or_else(|| JsErrorBox::generic("duplex channel already opened"))?;
  Ok(state.resource_table.add(TokioDuplexResource {
    inbound_rx: RefCell::new(channels.inbound_rx),
    outbound_tx: channels.outbound_tx,
  }))
}

#[op2]
#[string]
async fn op_duplex_read_line(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<String, JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TokioDuplexResource>(rid)
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  // P2: RefCell borrow instead of tokio::sync::Mutex::lock().await
  let value = {
    let mut rx = resource.inbound_rx.borrow_mut();
    rx.recv()
      .await
      .ok_or_else(|| JsErrorBox::generic("duplex channel reached EOF"))?
  };
  // Serialize Value → String for JS consumption.
  serde_json::to_string(&value).map_err(|err| JsErrorBox::generic(err.to_string()))
}

#[op2]
#[smi]
async fn op_duplex_write_line(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] line: String,
) -> Result<u32, JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TokioDuplexResource>(rid)
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  let written = line.len() as u32;
  // Parse incoming JSON string from JS into a Value so the Rust
  // receiver works with structured data directly (P0).
  let value: serde_json::Value =
    serde_json::from_str(&line).map_err(|err| JsErrorBox::generic(err.to_string()))?;
  resource
    .outbound_tx
    .send(value)
    .await
    .map_err(|err| JsErrorBox::generic(format!("duplex channel send failed: {err}")))?;
  Ok(written)
}

deno_core::extension!(
  libmainworker_duplex_ext,
  ops = [op_duplex_open, op_duplex_read_line, op_duplex_write_line],
  options = {
    channel_slot: DuplexChannelSlot,
  },
  state = |state, options| {
    state.put(options.channel_slot);
  }
);

pub(crate) fn duplex_extension(channels: DuplexChannelPair) -> deno_core::Extension {
  let mut ext = libmainworker_duplex_ext::init(DuplexChannelSlot {
    channels: Arc::new(Mutex::new(Some(channels))),
  });
  ext
    .esm_files
    .to_mut()
    .push(ExtensionFileSource::new_computed(
      DUPLEX_API_SPECIFIER,
      Arc::<str>::from(DUPLEX_API_SOURCE),
    ));
  ext.esm_entry_point = Some(DUPLEX_API_SPECIFIER);
  ext
}

// ─── Value-based helpers ────────────────────────────────────────────
// With `mpsc<serde_json::Value>` we avoid repeated to_string() / from_str()
// round-trips on the Rust side.

async fn send_value(
  tx: &mpsc::Sender<serde_json::Value>,
  value: serde_json::Value,
) -> Result<(), AnyError> {
  tx.send(value)
    .await
    .map_err(|err| AnyError::msg(format!("duplex channel send failed: {err}")))
}

async fn recv_value(
  rx: &mut mpsc::Receiver<serde_json::Value>,
) -> Result<serde_json::Value, AnyError> {
  rx.recv()
    .await
    .ok_or_else(|| AnyError::msg("duplex channel reached EOF"))
}

/// P3: Guard verbose debug output behind an env var so release builds
/// don't pay for formatting + I/O in the hot loop.
fn trace_duplex_enabled() -> bool {
  std::env::var("LIBMAINWORKER_DUPLEX_TRACE")
    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    .unwrap_or(false)
}

/// P1: Made async so future implementations can call async Rust services
/// (database queries, HTTP calls, etc.) without blocking the event loop.
async fn execute_ts_initiated_rust_call(
  payload: &serde_json::Value,
) -> Result<serde_json::Value, AnyError> {
  let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("echo");

  match op {
    "uppercase" => {
      let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AnyError::msg("rust_call.uppercase requires string field `text`"))?;
      Ok(serde_json::json!({
        "op": "uppercase",
        "output": text.to_uppercase(),
      }))
    }
    "sum" => {
      let values = payload
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AnyError::msg("rust_call.sum requires array field `values`"))?;
      let sum: f64 = values
        .iter()
        .map(|v| v.as_f64().ok_or_else(|| AnyError::msg("rust_call.sum expects numbers only")))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
      Ok(serde_json::json!({
        "op": "sum",
        "output": sum,
      }))
    }
    "echo" => Ok(serde_json::json!({
      "op": "echo",
      "output": payload,
    })),
    _ => Err(AnyError::msg(format!("unsupported rust_call op: {op}"))),
  }
}

fn normalize_external_process_message(line: &str) -> serde_json::Value {
  match serde_json::from_str::<serde_json::Value>(line) {
    Ok(serde_json::Value::Object(map)) => {
      if map.contains_key("type") {
        serde_json::Value::Object(map)
      } else {
        serde_json::json!({
          "type": "external_message",
          "payload": serde_json::Value::Object(map),
        })
      }
    }
    Ok(value) => serde_json::json!({
      "type": "external_message",
      "payload": value,
    }),
    Err(_) => serde_json::json!({
      "type": "external_message",
      "payload": line,
    }),
  }
}

/// The Rust-side event loop that bridges Rust ↔ TS via the duplex channels.
///
/// **P0 – persistent mode no longer wastes a 300 ms interval timer.**
/// In persistent mode the interval branch is disabled entirely via
/// `std::future::pending()`, yielding zero wakeups until a real message
/// arrives on one of the two remaining branches.
///
/// **P0 – channels carry `serde_json::Value` instead of `String`.**
/// **P1 – `execute_ts_initiated_rust_call` is async.**
/// **P3 – demo message removed; trace output gated behind env var.**
pub(crate) async fn rust_duplex_driver(
  rust_to_ts_tx: mpsc::Sender<serde_json::Value>,
  mut ts_to_rust_rx: mpsc::Receiver<serde_json::Value>,
  mut process_message_rx: mpsc::Receiver<String>,
  persistent: bool,
) -> Result<(), AnyError> {
  // P0: In persistent mode we never need the interval timer at all.
  // Using `Option<Interval>` prevents any 300 ms wakeup overhead.
  let mut maybe_interval = if persistent {
    None
  } else {
    let mut iv = tokio::time::interval(Duration::from_millis(300));
    iv.set_missed_tick_behavior(MissedTickBehavior::Skip);
    iv.tick().await; // consume the first immediate tick
    Some(iv)
  };

  let trace = trace_duplex_enabled();

  let mut ping_seq = 0_u64;
  let mut pong_count = 0_u64;
  let mut is_ready = false;
  let mut sent_shutdown = false;
  let mut process_input_closed = false;

  loop {
    tokio::select! {
      // In persistent mode `maybe_interval` is None, so we await
      // `pending()` — which never resolves — effectively disabling
      // this branch at zero cost.
      _ = async {
        match maybe_interval.as_mut() {
          Some(iv) => iv.tick().await,
          None => { std::future::pending::<tokio::time::Instant>().await }
        }
      }, if !sent_shutdown => {
        ping_seq += 1;
        send_value(
          &rust_to_ts_tx,
          serde_json::json!({
            "type": "ping",
            "seq": ping_seq,
            "from": "rust",
          }),
        )
        .await?;

        if is_ready && pong_count >= 3 {
          send_value(
            &rust_to_ts_tx,
            serde_json::json!({
              "type": "shutdown",
              "reason": "oneshot_completed",
            }),
          )
          .await?;
          sent_shutdown = true;
        } else if ping_seq >= 10 {
          send_value(
            &rust_to_ts_tx,
            serde_json::json!({
              "type": "shutdown",
              "reason": "oneshot_timeout",
            }),
          )
          .await?;
          sent_shutdown = true;
        }
      }
      inbound = recv_value(&mut ts_to_rust_rx) => {
        let message = match inbound {
          Ok(v) => v,
          Err(err) => {
            if sent_shutdown {
              break;
            }
            return Err(err);
          }
        };
        if trace {
          println!("[rust] received: {message}");
        }

        match message.get("type").and_then(|v| v.as_str()) {
          Some("ready") => {
            is_ready = true;
          }
          Some("pong") => {
            pong_count += 1;
            continue;
          }
          Some("message_result") => {
            if trace {
              if let Some(result) = message.get("result") {
                println!("[rust] message result: {result}");
              }
            }
          }
          Some("rust_call") => {
            let id = message.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let payload = message
              .get("payload")
              .cloned()
              .unwrap_or(serde_json::Value::Null);

            // P1: async rust_call — can now call async services
            match execute_ts_initiated_rust_call(&payload).await {
              Ok(result) => {
                send_value(
                  &rust_to_ts_tx,
                  serde_json::json!({
                    "type": "rust_call_result",
                    "id": id,
                    "result": result,
                  }),
                )
                .await?;
              }
              Err(err) => {
                send_value(
                  &rust_to_ts_tx,
                  serde_json::json!({
                    "type": "rust_call_error",
                    "id": id,
                    "error": err.to_string(),
                  }),
                )
                .await?;
              }
            }
          }
          Some("module_loaded") => {
            if trace {
              if let Some(specifier) = message.get("specifier") {
                println!("[rust] module loaded from ts: {specifier}");
              }
            }
          }
          Some("module_error") => {
            return Err(AnyError::msg(format!("ts module update failed: {message}")));
          }
          Some("mfa_updated") => {
            if trace {
              println!("[rust] mfa updated in ts: {}", message);
            }
          }
          Some("runtime_args_updated") => {
            if trace {
              println!("[rust] runtime args updated in ts: {}", message);
            }
          }
          Some("shutdown_ack") => {
            break;
          }
          Some("error") => {
            return Err(AnyError::msg(format!("ts message error: {message}")));
          }
          _ => {}
        }
      }
      external_line = process_message_rx.recv(), if !process_input_closed => {
        match external_line {
          Some(line) => {
            let outbound = normalize_external_process_message(&line);
            if outbound
              .get("type")
              .and_then(|v| v.as_str())
              .is_some_and(|value| value == "shutdown")
            {
              sent_shutdown = true;
            }
            send_value(&rust_to_ts_tx, outbound).await?;
          }
          None => {
            process_input_closed = true;
            if !sent_shutdown && is_ready {
              send_value(
                &rust_to_ts_tx,
                serde_json::json!({
                  "type": "shutdown",
                  "reason": "process_input_closed",
                }),
              )
              .await?;
              sent_shutdown = true;
            }
          }
        }
      }
    }
  }

  Ok(())
}
