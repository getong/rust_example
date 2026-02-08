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

  // Minimal, embed-friendly argument parsing.
  //
  // Supported format:
  //   `embed_deno <entry.ts|entry.js|jsr:pkg|npm:pkg> [-- <script args...>]`
  //
  // We intentionally do not expose the full Deno CLI flag surface. This keeps
  // `embed_deno` stable as an embedding target.
  let mut args: Vec<OsString> = env::args_os().collect();
  if args.len() < 2 {
    return Err(AnyError::msg(
      "Usage: embed_deno <entry.ts|entry.js|jsr:pkg|npm:pkg> [-- <script args...>]",
    ));
  }

  let _exe = args.remove(0);
  let script = args.remove(0);
  let script = script.to_string_lossy().to_string();

  // We don't support `deno run --allow-* <script>` style flags in this binary.
  // If the user accidentally passes a flag where the script should be, show a
  // helpful error instead of trying to parse it.
  if script == "--" || script.starts_with('-') {
    return Err(AnyError::msg(
      "embed_deno does not parse deno CLI flags. Usage: embed_deno <script> [-- <script args...>]",
    ));
  }

  // Script arguments are everything after the script; an optional leading `--`
  // is stripped (matching common CLI conventions).
  let script_args = if args.first().is_some_and(|a| a.to_string_lossy() == "--") {
    args.into_iter().skip(1).collect::<Vec<_>>()
  } else {
    args
  };

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
  //
  // Important: to use JSR / NPM (network downloads), the runtime requires
  // outbound network access. In the current sandbox this may fail even with
  // `--allow-all`.
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

    // For a smoother embedding experience, default to allowing everything.
    flags.permissions.allow_all = true;

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
