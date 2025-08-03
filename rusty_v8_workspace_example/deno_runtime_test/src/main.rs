#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

mod module_loader;

use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc, sync::Arc};

use colored::*;
use deno_resolver::npm::{ByonmInNpmPackageChecker, ByonmNpmResolver};
use deno_runtime::{
  deno_core::{ModuleSpecifier, error::AnyError, op2},
  deno_fs::RealFs,
  deno_permissions::{
    PermissionPrompter, Permissions, PermissionsContainer, PromptResponse, set_prompter,
  },
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use module_loader::TypescriptModuleLoader;
use sys_traits::impls::RealSys;

#[op2]
#[string]
fn example_custom_op(#[string] text: &str) -> String {
  println!("Hello {} from an op!", text);
  text.to_string() + " from Rust!"
}

deno_runtime::deno_core::extension!(
  example_extension,
  ops = [example_custom_op],
  esm_entry_point = "ext:example_extension/bootstrap.js",
  esm = [dir "src", "bootstrap.js"]
);

deno_runtime::deno_core::extension!(
  snapshot_options_extension,
  options = {
    snapshot_options: SnapshotOptions,
  },
  state = |state, options| {
    state.put::<SnapshotOptions>(options.snapshot_options);
  },
);

struct CustomPrompter;

impl PermissionPrompter for CustomPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    api_name: Option<&str>,
    is_unary: bool,
    _choices: Option<Box<dyn FnOnce() -> Vec<String> + Send + Sync>>,
  ) -> PromptResponse {
    println!(
      "{}\n{} {}\n{} {}\n{} {:?}\n{} {}",
      "Script is trying to access APIs and needs permission:"
        .yellow()
        .bold(),
      "Message:".bright_blue(),
      message,
      "Name:".bright_blue(),
      name,
      "API:".bright_blue(),
      api_name,
      "Is unary:".bright_blue(),
      is_unary
    );
    println!("Allow? [y/n]");

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
      match input.trim().to_lowercase().as_str() {
        "y" | "yes" => PromptResponse::Allow,
        _ => PromptResponse::Deny,
      }
    } else {
      println!("Failed to read input, denying permission");
      PromptResponse::Deny
    }
  }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
  let js_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("./test-files/all_the_things.ts");
  let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();

  let source_map_store = Rc::new(RefCell::new(HashMap::new()));

  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let permission_container =
    PermissionsContainer::new(permission_desc_parser, Permissions::none_with_prompt());

  set_prompter(Box::new(CustomPrompter));

  let snapshot_options = SnapshotOptions::default();

  let mut worker = MainWorker::bootstrap_from_options::<
    ByonmInNpmPackageChecker,
    ByonmNpmResolver<RealSys>,
    RealSys,
  >(
    &main_module,
    WorkerServiceOptions {
      module_loader: Rc::new(TypescriptModuleLoader {
        source_maps: source_map_store,
      }),
      // File-only loader
      // module_loader: Rc::new(FsModuleLoader),
      permissions: permission_container,
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      node_services: None,
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      fs: fs.clone(),
      deno_rt_native_addon_loader: Default::default(),
      fetch_dns_resolver: Default::default(),
    },
    WorkerOptions {
      extensions: vec![
        snapshot_options_extension::init(snapshot_options),
        example_extension::init(),
      ],
      ..Default::default()
    },
  );
  worker.execute_main_module(&main_module).await?;
  worker.run_event_loop(false).await?;

  println!("Exit code: {}", worker.exit_code());

  Ok(())
}
