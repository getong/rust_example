use std::{env, path::PathBuf};

use deno_core::extension;

fn main() {
  extension!(
    // extension name
    runjs,
    // list of all JS files in the extension
    esm_entry_point = "ext:runjs/src/runtime.js",
    // the entrypoint to our extension
    esm = ["src/runtime.js"]
  );

  let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
  let snapshot_path = out_dir.join("RUNJS_SNAPSHOT.bin");

  let snapshot = deno_core::snapshot::create_snapshot(
    deno_core::snapshot::CreateSnapshotOptions {
      cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
      startup_snapshot: None,
      skip_op_registration: false,
      extensions: vec![runjs::init_ops_and_esm()],
      with_runtime_cb: None,
      extension_transpiler: None,
    },
    None,
  )
  .unwrap();

  std::fs::write(snapshot_path, snapshot.output).unwrap();
}
