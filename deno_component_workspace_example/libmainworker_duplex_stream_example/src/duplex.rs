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

pub(crate) struct DuplexChannelPair {
  pub(crate) inbound_rx: mpsc::Receiver<String>,
  pub(crate) outbound_tx: mpsc::Sender<String>,
}

#[derive(Clone)]
struct DuplexChannelSlot {
  channels: Arc<Mutex<Option<DuplexChannelPair>>>,
}

#[derive(Debug)]
struct TokioDuplexResource {
  inbound_rx: tokio::sync::Mutex<mpsc::Receiver<String>>,
  outbound_tx: mpsc::Sender<String>,
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
    inbound_rx: tokio::sync::Mutex::new(channels.inbound_rx),
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
  let mut inbound_rx = resource.inbound_rx.lock().await;
  read_line(&mut inbound_rx)
    .await
    .map_err(|err| JsErrorBox::generic(err.to_string()))
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
  write_line(&resource.outbound_tx, line)
    .await
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
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

async fn read_line(rx: &mut mpsc::Receiver<String>) -> Result<String, AnyError> {
  rx.recv()
    .await
    .ok_or_else(|| AnyError::msg("duplex channel reached EOF"))
}

async fn write_line(tx: &mpsc::Sender<String>, line: String) -> Result<(), AnyError> {
  tx.send(line)
    .await
    .map_err(|err| AnyError::msg(format!("duplex channel send failed: {err}")))
}

async fn write_json_line(
  tx: &mpsc::Sender<String>,
  value: &serde_json::Value,
) -> Result<(), AnyError> {
  let line = serde_json::to_string(value).map_err(|err| AnyError::msg(err.to_string()))?;
  write_line(tx, line).await
}

fn execute_ts_initiated_rust_call(
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
      let mut sum = 0.0_f64;
      for value in values {
        let number = value
          .as_f64()
          .ok_or_else(|| AnyError::msg("rust_call.sum expects numbers only"))?;
        sum += number;
      }
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

pub(crate) async fn rust_duplex_driver(
  rust_to_ts_tx: mpsc::Sender<String>,
  mut ts_to_rust_rx: mpsc::Receiver<String>,
) -> Result<(), AnyError> {
  let mut interval = tokio::time::interval(Duration::from_millis(300));
  interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
  interval.tick().await;

  let mut ping_seq = 0_u64;
  let mut pong_count = 0_u64;
  let mut is_ready = false;
  let mut sent_demo_message = false;
  let mut sent_shutdown = false;

  loop {
    tokio::select! {
      _ = interval.tick(), if !sent_shutdown => {
        ping_seq += 1;
        write_json_line(
          &rust_to_ts_tx,
          &serde_json::json!({
            "type": "ping",
            "seq": ping_seq,
            "from": "rust",
          }),
        )
        .await?;

        if is_ready && !sent_demo_message {
          write_json_line(
            &rust_to_ts_tx,
            &serde_json::json!({
              "type": "message",
              "id": "demo-1",
              "payload": {
                "text": "hello from rust",
                "seq": ping_seq,
              }
            }),
          )
          .await?;
          sent_demo_message = true;
        }

        if is_ready && pong_count >= 3 {
          write_json_line(
            &rust_to_ts_tx,
            &serde_json::json!({
              "type": "shutdown",
              "reason": "demo_completed",
            }),
          )
          .await?;
          sent_shutdown = true;
        } else if ping_seq >= 10 {
          write_json_line(
            &rust_to_ts_tx,
            &serde_json::json!({
              "type": "shutdown",
              "reason": "timeout",
            }),
          )
          .await?;
          sent_shutdown = true;
        }
      }
      inbound = read_line(&mut ts_to_rust_rx) => {
        let inbound = inbound?;
        println!("[rust] received: {inbound}");
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&inbound) else {
          continue;
        };

        match message.get("type").and_then(|v| v.as_str()) {
          Some("ready") => {
            is_ready = true;
          }
          Some("pong") => {
            pong_count += 1;
          }
          Some("message_result") => {
            if let Some(result) = message.get("result") {
              println!("[rust] message result: {result}");
            }
          }
          Some("rust_call") => {
            let id = message.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let payload = message
              .get("payload")
              .cloned()
              .unwrap_or(serde_json::Value::Null);

            match execute_ts_initiated_rust_call(&payload) {
              Ok(result) => {
                write_json_line(
                  &rust_to_ts_tx,
                  &serde_json::json!({
                    "type": "rust_call_result",
                    "id": id,
                    "result": result,
                  }),
                )
                .await?;
              }
              Err(err) => {
                write_json_line(
                  &rust_to_ts_tx,
                  &serde_json::json!({
                    "type": "rust_call_error",
                    "id": id,
                    "error": err.to_string(),
                  }),
                )
                .await?;
              }
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
    }
  }

  Ok(())
}
