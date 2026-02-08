// Copyright 2018-2026 the Deno authors. MIT license.

use std::{
  env,
  ffi::OsString,
  io::{BufRead, Write},
  sync::Arc,
};

use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use embed_deno::embed::EmbedResult;
use serde_json::Value as JsonValue;

fn main() {
  if let Err(error) = run() {
    eprintln!("{error:#}");
    std::process::exit(1);
  }
}

fn run() -> Result<(), AnyError> {
  let exit_code = run_inner()?;
  std::process::exit(exit_code);
}

fn run_inner() -> Result<i32, AnyError> {
  // Required by rustls 0.23+ (used throughout the vendored Deno CLI stack).
  // Deno's CLI does this early in startup.
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  // Ensure we always have a writable `DENO_DIR`, otherwise parts of Deno's
  // cache (like the V8 code cache sqlite db) may fail in sandboxed/read-only
  // environments.
  if env::var_os("DENO_DIR").is_none() {
    let deno_dir = std::env::temp_dir().join("embed_deno_deno_dir");
    std::fs::create_dir_all(&deno_dir)?;
    // SAFETY: setting env vars is safe at process startup.
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      std::env::set_var("DENO_DIR", &deno_dir);
    }
  }

  // JSR registry base URL can be overridden via `JSR_URL`.
  // Example (if jsr.io is not reachable):
  //   `JSR_URL=https://your-mirror/ CC=clang cargo run -p embed_deno -- main.ts`
  //
  // We don't set a default here because the vendored Deno resolver already
  // defaults to `https://jsr.io/`.

  // Minimal, embed-friendly argument parsing.
  //
  // Supported formats:
  //   1) One-shot (legacy): `embed_deno <entry.ts|entry.js|jsr:pkg|npm:pkg> [-- <script args...>]`
  //   2) Persistent daemon (recommended): `embed_deno --daemon [-- <script argv...>]` then send
  //      JSON requests on stdin, one per line.
  let mut args: Vec<OsString> = env::args_os().collect();
  if args.len() < 2 {
    return Err(AnyError::msg(
      "Usage: embed_deno <script> [-- <script args...>]\n       embed_deno --daemon [-- <script \
       argv...>]",
    ));
  }

  let _exe = args.remove(0);
  let mode = args.remove(0);
  let mode = mode.to_string_lossy().to_string();

  if mode == "--daemon" {
    // Daemon format:
    //   `embed_deno --daemon [<preload_module>] [-- <argv...>]`
    //
    // If `<preload_module>` is provided, it will be imported once at startup
    // (so it can print logs / initialize state) and then cached for `call_id`.
    let mut daemon_args = args;
    let mut preload_module: Option<String> = None;
    if let Some(first) = daemon_args.first() {
      let first = first.to_string_lossy().to_string();
      if first != "--" && !first.starts_with('-') {
        preload_module = Some(first);
        daemon_args.remove(0);
      }
    }
    if daemon_args
      .first()
      .is_some_and(|a| a.to_string_lossy() == "--")
    {
      daemon_args.remove(0);
    }
    let argv = daemon_args
      .into_iter()
      .map(|arg| arg.to_string_lossy().to_string())
      .collect::<Vec<_>>();

    let embed_result = Arc::new(std::sync::Mutex::new(EmbedResult::default()));
    let handle = embed_deno::runtime::spawn_runtime(embed_deno::runtime::DenoRuntimeOptions {
      initial_cwd: std::env::current_dir().ok(),
      argv,
      roots: LibWorkerFactoryRoots::default(),
      embed_result,
    })?;

    if let Some(preload) = preload_module {
      let r = handle.load_module(preload)?;
      let module_id = r.module_id;
      let embed_result = r.embed_result;
      let embed_exit_data = r.embed_exit_data;

      // Print a preload response on stdout so `--daemon <module>` has an
      // observable "result" like the one-shot mode.
      println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
          "ok": true,
          "op": "preload",
          "module_id": module_id,
          "embed_result": embed_result,
          "embed_exit_data": embed_exit_data,
        }))?
      );
      let _ = std::io::stdout().flush();

      eprintln!("preloaded module_id={module_id}");
    }
    eprintln!("embed_deno daemon ready (send JSON lines on stdin)");

    // Protocol: one JSON object per line on stdin.
    //   {"op":"call","module":"./function_caller.ts","function":"greet","args":["Bob"]}
    //   {"op":"call_id","module_id":1,"function":"add","args":[1,2]}
    //   {"op":"load","module":"./function_caller.ts"}
    //   {"op":"shutdown"}
    let stdin = std::io::stdin();
    let mut stdin_lock = stdin.lock();
    let mut line = String::new();
    loop {
      line.clear();
      let n = stdin_lock.read_line(&mut line)?;
      if n == 0 {
        break;
      }
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }

      let req: JsonValue = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(err) => {
          println!(
            "{}",
            serde_json::json!({"ok":false,"error":format!("invalid json: {err}")})
          );
          continue;
        }
      };

      let op = req.get("op").and_then(|v| v.as_str()).unwrap_or("call");
      let resp = match op {
        "load" => {
          let module = req
            .get("module")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnyError::msg("missing 'module'"))?;
          let r = handle.load_module(module)?;
          serde_json::json!({
            "ok": true,
            "module_id": r.module_id,
            "embed_result": r.embed_result,
            "embed_exit_data": r.embed_exit_data,
          })
        }
        "call_id" => {
          let module_id = req
            .get("module_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| AnyError::msg("missing 'module_id'"))? as u32;
          let function = req
            .get("function")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnyError::msg("missing 'function'"))?;
          let args = req.get("args").cloned().unwrap_or(JsonValue::Array(vec![]));
          let r = handle.call_module_function_by_id(module_id, function, args)?;
          serde_json::json!({
            "ok": true,
            "module_id": r.module_id,
            "result": r.result,
            "embed_result": r.embed_result,
            "embed_exit_data": r.embed_exit_data,
          })
        }
        "shutdown" => {
          handle.shutdown();
          serde_json::json!({"ok": true, "shutdown": true})
        }
        "call" | _ => {
          let module = req
            .get("module")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnyError::msg("missing 'module'"))?;
          let function = req
            .get("function")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnyError::msg("missing 'function'"))?;
          let args = req.get("args").cloned().unwrap_or(JsonValue::Array(vec![]));
          let r = handle.call_module_function(module, function, args)?;
          serde_json::json!({
            "ok": true,
            "module_id": r.module_id,
            "result": r.result,
            "embed_result": r.embed_result,
            "embed_exit_data": r.embed_exit_data,
          })
        }
      };

      let out = match serde_json::to_string(&resp) {
        Ok(s) => s,
        Err(err) => {
          serde_json::to_string(&serde_json::json!({"ok":false,"error":err.to_string()}))?
        }
      };
      println!("{out}");

      if op == "shutdown" {
        break;
      }
    }

    Ok(0)
  } else {
    // One-shot script execution (legacy behavior).
    let script = mode;

    // We don't support `deno run --allow-* <script>` style flags in this binary.
    if script == "--" || script.starts_with('-') {
      return Err(AnyError::msg(
        "embed_deno does not parse deno CLI flags. Usage: embed_deno <script> [-- <script \
         args...>]",
      ));
    }

    let script_args = if args.first().is_some_and(|a| a.to_string_lossy() == "--") {
      args.into_iter().skip(1).collect::<Vec<_>>()
    } else {
      args
    };

    let embed_result = Arc::new(std::sync::Mutex::new(EmbedResult::default()));
    let embed_result_for_runtime = embed_result.clone();

    let roots = LibWorkerFactoryRoots::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()?;
    let exit_code = runtime.block_on(async move {
      let argv = script_args
        .into_iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

      let mut flags = embed_deno::args::Flags::default();
      flags.initial_cwd = std::env::current_dir().ok();
      flags.argv = argv;
      flags.subcommand = embed_deno::args::DenoSubcommand::Run(embed_deno::args::RunFlags {
        script,
        ..Default::default()
      });
      flags.permissions.allow_all = true;

      embed_deno::tools::run::run_script_with_extension(
        deno_runtime::WorkerExecutionMode::Run,
        Arc::new(flags),
        None,
        None,
        roots,
        embed_deno::embed::extension(embed_result_for_runtime),
      )
      .await
    })?;

    if let Ok(mut guard) = embed_result.lock() {
      if let Some(json) = guard.exit_data.take() {
        println!("EMBED_DENO_EXIT_DATA={json}");
      }
      if let Some(json) = guard.result.take() {
        println!("EMBED_DENO_RESULT={json}");
      }
    }

    Ok(exit_code)
  }
}
