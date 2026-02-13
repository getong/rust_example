use std::{
  borrow::Cow,
  cell::RefCell,
  path::PathBuf,
  rc::Rc,
  sync::{Arc, Mutex},
};

use deno_core::{OpState, Resource, ResourceId, error::AnyError, op2};
use deno_error::JsErrorBox;
use deno_runtime::WorkerExecutionMode;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt, DuplexStream},
  time::{Duration, MissedTickBehavior},
};
use url::Url;

#[derive(Clone)]
struct DuplexStreamSlot {
  stream: Arc<Mutex<Option<DuplexStream>>>,
}

#[derive(Clone, Default)]
struct EmbedResult {
  result: Option<String>,
  exit_data: Option<String>,
}

#[derive(Debug)]
struct TokioDuplexResource {
  stream: tokio::sync::Mutex<DuplexStream>,
}

impl Resource for TokioDuplexResource {
  fn name(&self) -> Cow<'_, str> {
    "mainworkerDuplex".into()
  }
}

#[op2(fast)]
#[smi]
fn op_duplex_open(state: &mut OpState) -> Result<ResourceId, JsErrorBox> {
  let slot = state.borrow::<DuplexStreamSlot>().clone();
  let mut guard = slot
    .stream
    .lock()
    .map_err(|_| JsErrorBox::generic("failed to lock duplex stream slot"))?;
  let stream = guard
    .take()
    .ok_or_else(|| JsErrorBox::generic("duplex stream already opened"))?;
  Ok(state.resource_table.add(TokioDuplexResource {
    stream: tokio::sync::Mutex::new(stream),
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
  let mut stream = resource.stream.lock().await;
  read_line(&mut stream)
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
  let mut stream = resource.stream.lock().await;
  write_line(&mut stream, &line)
    .await
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  Ok(line.len() as u32)
}

deno_core::extension!(
  libmainworker_duplex_ext,
  ops = [op_duplex_open, op_duplex_read_line, op_duplex_write_line],
  esm_entry_point = "ext:libmainworker_duplex_ext/duplex_api.js",
  esm = [dir "src", "duplex_api.js"],
  options = {
    stream_slot: DuplexStreamSlot,
  },
  state = |state, options| {
    state.put(options.stream_slot);
  }
);

fn duplex_extension(stream: DuplexStream) -> deno_core::Extension {
  libmainworker_duplex_ext::init(DuplexStreamSlot {
    stream: Arc::new(Mutex::new(Some(stream))),
  })
}

#[op2(fast)]
fn libmainworker_embed_set_result(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.result = Some(value);
  }
}

#[op2(fast)]
fn libmainworker_embed_set_exit_data(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.exit_data = Some(value);
  }
}

deno_core::extension!(
  libmainworker_embed_ext,
  ops = [libmainworker_embed_set_result, libmainworker_embed_set_exit_data],
  esm_entry_point = "ext:libmainworker_embed_ext/embed_result.js",
  esm = [dir "src", "embed_result.js"],
  options = {
    result_holder: Arc<Mutex<EmbedResult>>,
  },
  state = |state, options| {
    state.put(options.result_holder);
  }
);

fn embed_extension(result_holder: Arc<Mutex<EmbedResult>>) -> deno_core::Extension {
  libmainworker_embed_ext::init(result_holder)
}

async fn read_line(stream: &mut DuplexStream) -> Result<String, AnyError> {
  let mut buf = Vec::new();
  loop {
    let mut byte = [0_u8; 1];
    let read = stream.read(&mut byte).await?;
    if read == 0 {
      break;
    }
    if byte[0] == b'\n' {
      break;
    }
    buf.push(byte[0]);
  }

  if buf.is_empty() {
    return Err(AnyError::msg("duplex stream reached EOF"));
  }

  String::from_utf8(buf).map_err(|err| AnyError::msg(err.to_string()))
}

async fn write_line(stream: &mut DuplexStream, line: &str) -> Result<(), AnyError> {
  stream.write_all(line.as_bytes()).await?;
  stream.write_all(b"\n").await?;
  stream.flush().await?;
  Ok(())
}

async fn write_json_line(
  stream: &mut DuplexStream,
  value: &serde_json::Value,
) -> Result<(), AnyError> {
  let line = serde_json::to_string(value).map_err(|err| AnyError::msg(err.to_string()))?;
  write_line(stream, &line).await
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

async fn rust_duplex_driver(mut stream: DuplexStream) -> Result<(), AnyError> {
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
          &mut stream,
          &serde_json::json!({
            "type": "ping",
            "seq": ping_seq,
            "from": "rust",
          }),
        )
        .await?;

        if is_ready && !sent_demo_message {
          write_json_line(
            &mut stream,
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
            &mut stream,
            &serde_json::json!({
              "type": "shutdown",
              "reason": "demo_completed",
            }),
          )
          .await?;
          sent_shutdown = true;
        } else if ping_seq >= 10 {
          write_json_line(
            &mut stream,
            &serde_json::json!({
              "type": "shutdown",
              "reason": "timeout",
            }),
          )
          .await?;
          sent_shutdown = true;
        }
      }
      inbound = read_line(&mut stream) => {
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
                  &mut stream,
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
                  &mut stream,
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

fn resolve_target_specifier(arg: &str) -> Result<String, AnyError> {
  let is_url_like = arg.starts_with("file://")
    || arg.starts_with("http://")
    || arg.starts_with("https://")
    || arg.starts_with("jsr:")
    || arg.starts_with("npm:");

  if is_url_like {
    return Ok(arg.to_string());
  }

  let path = PathBuf::from(arg);
  let abs_path = if path.is_absolute() {
    path
  } else {
    std::env::current_dir()?.join(path)
  };

  Url::from_file_path(&abs_path)
    .map(|url| url.to_string())
    .map_err(|_| {
      AnyError::msg(format!(
        "failed to convert path to file url: {}",
        abs_path.display()
      ))
    })
}

fn bootstrap_script_path() -> Result<PathBuf, AnyError> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("src")
    .join("duplex_bootstrap.ts");
  if !path.exists() {
    return Err(AnyError::msg(format!(
      "duplex bootstrap script not found: {}",
      path.display()
    )));
  }
  Ok(path)
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let (result_tx, result_rx) = tokio::sync::oneshot::channel::<Result<(), AnyError>>();

  std::thread::Builder::new()
    .name("libmainworker_duplex_runtime".to_string())
    .spawn(move || {
      let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
      {
        Ok(runtime) => runtime,
        Err(err) => {
          let _ = result_tx.send(Err(AnyError::msg(format!(
            "failed to create current-thread runtime: {err}"
          ))));
          return;
        }
      };

      let local = tokio::task::LocalSet::new();
      let result = local.block_on(&runtime, run_inner());
      let _ = result_tx.send(result);
    })
    .map_err(|err| AnyError::msg(format!("failed to spawn runtime thread: {err}")))?;

  result_rx
    .await
    .map_err(|err| AnyError::msg(format!("runtime thread dropped result: {err}")))?
}

async fn run_inner() -> Result<(), AnyError> {
  // Required by rustls 0.23+ when TLS-backed APIs (for example fetch) are used.
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("DENO_FORCE_OP_REGISTRATION", "1");
  }

  let target_arg = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "embed_deno/simple_main.ts".to_string());
  let target_specifier = resolve_target_specifier(&target_arg)?;
  let bootstrap_path = bootstrap_script_path()?;

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("LIBMAINWORKER_TARGET_SPECIFIER", &target_specifier);
  }

  println!("target script: {target_arg}");
  println!("target specifier: {target_specifier}");

  let (rust_stream, js_stream) = tokio::io::duplex(16 * 1024);
  let embed_result = Arc::new(Mutex::new(EmbedResult::default()));
  let embed_result_for_worker = embed_result.clone();

  let mut flags = deno::args::Flags::default();
  flags.initial_cwd = std::env::current_dir().ok();
  flags.permissions.allow_all = true;
  flags.cached_only = false;
  flags.no_remote = false;
  flags.subcommand = deno::args::DenoSubcommand::Run(deno::args::RunFlags {
    script: bootstrap_path.to_string_lossy().to_string(),
    ..Default::default()
  });

  let flags = Arc::new(flags);
  let factory = deno::CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;

  let main_module = cli_options.resolve_main_module()?;
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  deno::tools::run::maybe_npm_install(&factory).await?;

  let worker_factory = factory
    .create_cli_main_worker_factory_with_roots(Default::default())
    .await?;

  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Run,
      main_module.clone(),
      preload_modules,
      require_modules,
      factory.root_permissions_container()?.clone(),
      vec![
        duplex_extension(js_stream),
        embed_extension(embed_result_for_worker),
      ],
      Default::default(),
      None,
    )
    .await?;

  println!("mainworker created with CLI factory + duplex extension");
  let mut worker_future = std::pin::pin!(worker.run());
  let mut driver_future = std::pin::pin!(rust_duplex_driver(rust_stream));
  let mut maybe_exit_code: Option<i32> = None;
  let mut driver_completed = false;

  loop {
    tokio::select! {
      worker_result = &mut worker_future, if maybe_exit_code.is_none() => {
        maybe_exit_code = Some(worker_result?);
        if driver_completed {
          break;
        }
      }
      driver_result = &mut driver_future, if !driver_completed => {
        driver_result?;
        driver_completed = true;
        if maybe_exit_code.is_some() {
          break;
        }
      }
    }
  }

  let exit_code =
    maybe_exit_code.ok_or_else(|| AnyError::msg("worker finished without exit code"))?;
  if !driver_completed {
    return Err(AnyError::msg("rust duplex driver did not complete"));
  }

  println!("worker exit code: {exit_code}");
  println!("rust <-> ts duplex communication completed");

  if let Ok(mut guard) = embed_result.lock() {
    if let Some(json) = guard.exit_data.take() {
      println!("EMBED_DENO_EXIT_DATA={json}");
    }
    if let Some(json) = guard.result.take() {
      println!("EMBED_DENO_RESULT={json}");
    }
  }

  Ok(())
}
