use std::{
  env,
  ffi::OsString,
  io::{BufRead, Write},
  sync::Arc,
};

mod embed;
mod runtime;
use serde_json::Value as JsonValue;

fn main() {
  if let Err(error) = run() {
    eprintln!("{error:#}");
    if std::env::var("LIBMAINWORKER_DEBUG").ok().as_deref() == Some("1") {
      eprintln!("debug_error={error:?}");
    }
    std::process::exit(1);
  }
}

fn run() -> Result<(), deno_core::error::AnyError> {
  let exit_code = run_inner()?;
  std::process::exit(exit_code);
}

fn run_inner() -> Result<i32, deno_core::error::AnyError> {
  // Required by rustls 0.23+ (used throughout the vendored Deno CLI stack).
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  // `deno`'s CLI stack enables an optimization for `deno run` that skips
  // registering ops because the snapshot already contains all built-in ops.
  //
  // Embedding use-cases (like this example) inject custom ops at runtime,
  // therefore we must ensure ops are registered.
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("DENO_FORCE_OP_REGISTRATION", "1");
  }

  if env::var_os("DENO_DIR").is_none() {
    // Keep the cache near the project so it can be reused for offline runs.
    let deno_dir = std::env::current_dir()
      .unwrap_or_else(|_| std::path::PathBuf::from("."))
      .join(".libmainworker_deno_dir");
    std::fs::create_dir_all(&deno_dir)?;
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      std::env::set_var("DENO_DIR", &deno_dir);
    }
  }

  // Minimal argument parsing.
  //
  // Usage:
  //   - One-shot (online by default):
  //       `libmainworker_example [--offline|--online] <script.ts|script.js|jsr:pkg|npm:pkg> [-- <argv...>]`
  //   - Daemon (online by default):
  //       `libmainworker_example [--offline|--online] --daemon [<preload>] [-- <argv...>]`
  //   - Pre-cache deps (requires network):
  //       `libmainworker_example --cache <entry.ts|jsr:pkg|npm:pkg>`
  let mut args: Vec<OsString> = env::args_os().collect();
  if args.len() < 2 {
    return Err(deno_core::error::AnyError::msg(
      "Usage: libmainworker_example [--offline|--online] <script> [-- <script args...>]\n       \
libmainworker_example [--offline|--online] --daemon [-- <script argv...>]\n       \
libmainworker_example --cache <entry>",
    ));
  }
  let _exe = args.remove(0);

  // Default: online so `jsr:` / `npm:` can download.
  let mut offline = false;

  let mut mode = args.remove(0).to_string_lossy().to_string();
  if mode == "--offline" {
    offline = true;
    if args.is_empty() {
      return Err(deno_core::error::AnyError::msg(
        "Usage: libmainworker_example --offline <script>|--daemon",
      ));
    }
    mode = args.remove(0).to_string_lossy().to_string();
  } else if mode == "--online" {
    offline = false;
    if args.is_empty() {
      return Err(deno_core::error::AnyError::msg(
        "Usage: libmainworker_example --online <script>|--daemon",
      ));
    }
    mode = args.remove(0).to_string_lossy().to_string();
  }

  // If this environment has proxy variables set, Deno will use them.
  // That is often desired, but it can also break downloads when the proxy
  // endpoint is not reachable. Provide a simple escape hatch.
  if env::var_os("LIBMAINWORKER_IGNORE_PROXY").is_some() {
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      std::env::remove_var("ALL_PROXY");
      std::env::remove_var("HTTP_PROXY");
      std::env::remove_var("HTTPS_PROXY");
      std::env::remove_var("NO_PROXY");
    }
  }

  // Offline pre-cache mode (requires network on first run):
  //   libmainworker_example --cache <entry.ts|jsr:...|npm:...>
  if mode == "--cache" {
    let Some(entry) = args.first() else {
      return Err(deno_core::error::AnyError::msg(
        "Usage: libmainworker_example --cache <entry>",
      ));
    };
    let entry = entry.to_string_lossy().to_string();
    let resolved_entry = if entry.starts_with("./")
      || entry.starts_with("../")
      || entry.ends_with(".ts")
      || entry.ends_with(".js")
      || entry.ends_with(".mts")
      || entry.ends_with(".cts")
    {
      let p = std::path::PathBuf::from(&entry);
      if p.is_absolute() {
        entry
      } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        cwd.join(p).to_string_lossy().to_string()
      }
    } else {
      entry
    };

    let mut flags = deno::args::Flags::default();
    flags.initial_cwd = std::env::current_dir().ok();
    flags.permissions.allow_all = true;
    flags.cached_only = false;
    flags.no_remote = false;
    flags.subcommand = deno::args::DenoSubcommand::Cache(deno::args::CacheFlags {
      files: vec![resolved_entry.clone()],
    });
    let rt = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()?;
    rt.block_on(async move {
      deno::tools::installer::install_from_entrypoints(
        Arc::new(flags),
        deno::args::InstallEntrypointsFlags {
          entrypoints: vec![resolved_entry],
          lockfile_only: false,
        },
      )
      .await
    })?;
    eprintln!("cache ok (now run with --offline)");
    return Ok(0);
  }

  if mode == "--daemon" {
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

    let embed_result = Arc::new(std::sync::Mutex::new(embed::EmbedResult::default()));
    let handle = runtime::spawn_runtime(runtime::DenoRuntimeOptions {
      initial_cwd: std::env::current_dir().ok(),
      argv,
      roots: Default::default(),
      embed_result,
      cached_only: offline,
      no_remote: offline,
    })?;

    if let Some(preload) = preload_module {
      let r = handle.load_module(preload)?;
      println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
          "ok": true,
          "op": "preload",
          "module_id": r.module_id,
          "embed_result": r.embed_result,
          "embed_exit_data": r.embed_exit_data,
        }))?
      );
      let _ = std::io::stdout().flush();
    }

    if offline {
      eprintln!("libmainworker_example daemon ready (offline, JSON lines on stdin)");
    } else {
      eprintln!("libmainworker_example daemon ready (online, JSON lines on stdin)");
    }

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
            .ok_or_else(|| deno_core::error::AnyError::msg("missing 'module'"))?;
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
            .ok_or_else(|| deno_core::error::AnyError::msg("missing 'module_id'"))?
            as u32;
          let function = req
            .get("function")
            .and_then(|v| v.as_str())
            .ok_or_else(|| deno_core::error::AnyError::msg("missing 'function'"))?;
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
            .ok_or_else(|| deno_core::error::AnyError::msg("missing 'module'"))?;
          let function = req
            .get("function")
            .and_then(|v| v.as_str())
            .ok_or_else(|| deno_core::error::AnyError::msg("missing 'function'"))?;
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
      let _ = std::io::stdout().flush();

      if op == "shutdown" {
        break;
      }
    }

    Ok(0)
  } else {
    let script = mode;
    if script == "--" || script.starts_with('-') {
      return Err(deno_core::error::AnyError::msg(
        "libmainworker_example does not parse deno CLI flags. Usage: libmainworker_example \
         <script> [-- <script args...>]",
      ));
    }

    let script_args = if args.first().is_some_and(|a| a.to_string_lossy() == "--") {
      args.into_iter().skip(1).collect::<Vec<_>>()
    } else {
      args
    };

    // Make relative specifiers work like `deno run` when invoked from
    // within this workspace.
    //
    // Example: `cargo run -p libmainworker_example -- libmainworker_example/fetch_api_example.ts`
    // should resolve against current working directory.
    let script = if script.starts_with("./")
      || script.starts_with("../")
      || script.ends_with(".ts")
      || script.ends_with(".js")
      || script.ends_with(".mts")
      || script.ends_with(".cts")
    {
      let p = std::path::PathBuf::from(&script);
      if p.is_absolute() {
        script
      } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        cwd.join(p).to_string_lossy().to_string()
      }
    } else {
      script
    };

    let embed_result = Arc::new(std::sync::Mutex::new(embed::EmbedResult::default()));
    let embed_result_for_runtime = embed_result.clone();

    let roots = Default::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()?;
    let exit_code = runtime.block_on(async move {
      let argv = script_args
        .into_iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

      let mut flags = deno::args::Flags::default();
      flags.initial_cwd = std::env::current_dir().ok();
      flags.argv = argv;
      flags.cached_only = offline;
      flags.no_remote = offline;
      flags.subcommand = deno::args::DenoSubcommand::Run(deno::args::RunFlags {
        script,
        ..Default::default()
      });
      flags.permissions.allow_all = true;

      deno::tools::run::run_script_with_extension(
        deno_runtime::WorkerExecutionMode::Run,
        Arc::new(flags),
        None,
        None,
        roots,
        embed::extension(embed_result_for_runtime),
      )
      .await
    })?;

    if let Ok(mut guard) = embed_result.lock() {
      if let Some(json) = guard.exit_data.take() {
        println!("LIBMAINWORKER_EXIT_DATA={json}");
      }
      if let Some(json) = guard.result.take() {
        println!("LIBMAINWORKER_RESULT={json}");
      }
    }

    Ok(exit_code)
  }
}
