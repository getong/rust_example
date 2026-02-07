// Copyright 2018-2026 the Deno authors. MIT license.

use std::{env, ffi::OsString, sync::Arc};

use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::WorkerExecutionMode;
use embed_deno::embed::EmbedResult;

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

  // We want `embed_deno <script.ts>` to behave like `deno run <script.ts>`.
  // Additionally, for a smoother embedding experience, default to allowing
  // all permissions (matching `--allow-all`). This avoids interactive
  // permission prompts when running embedded scripts.
  //
  // Users can still explicitly override permissions by passing any
  // `--allow-*`, `--deny-*`, or `--no-prompt` flags.
  let mut args: Vec<OsString> = env::args_os().collect();
  if args.len() < 2 {
    return Err(AnyError::msg(
      "Usage: embed_deno <entry.ts|entry.js|jsr:pkg|npm:pkg> [-- <script args...>]",
    ));
  }

  let mut deno_style_args = Vec::with_capacity(args.len() + 2);
  deno_style_args.push(args.remove(0));
  deno_style_args.push(OsString::from("run"));

  // Only treat arguments before the script specifier as Deno flags.
  // This avoids mis-detecting script arguments like `-- --allow-net`.
  let script_index = args
    .iter()
    .position(|arg| {
      let s = arg.to_string_lossy();
      s == "--" || !s.starts_with('-')
    })
    .unwrap_or(args.len());
  let has_permission_flag = args[.. script_index].iter().any(|arg| {
    let arg = arg.to_string_lossy();
    arg == "-A" || arg.starts_with("--allow-") || arg.starts_with("--deny-") || arg == "--no-prompt"
  });
  if !has_permission_flag {
    deno_style_args.push(OsString::from("--allow-all"));
  }

  deno_style_args.extend(args);

  // Collect structured data from inside the runtime.
  let embed_result = Arc::new(std::sync::Mutex::new(EmbedResult::default()));
  let embed_result_for_runtime = embed_result.clone();

  // Parse flags, initialize V8, and build the worker (mostly Deno CLI behavior).
  // For maximal compatibility with `deno run`, reuse the vendored `tools::run`
  // entrypoint, which will initialize V8/logging/permissions and use the full
  // CliFactory-based module loader + resolver stack.
  //
  // Note: we inject `embed_deno::embed` as a custom extension so scripts can
  // send structured results back to this process.
  let roots = LibWorkerFactoryRoots::default();
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;
  let exit_code = runtime.block_on(async move {
    let flags = embed_deno::args::flags_from_vec_with_initial_cwd(
      deno_style_args,
      std::env::current_dir().ok(),
    )?;

    // HACK: For now we inject the extension globally, so the normal `run_script`
    // worker creation path picks it up without requiring changes throughout the
    // factory.
    embed_deno::tools::run::run_script_with_extension(
      WorkerExecutionMode::Run,
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
