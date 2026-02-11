use std::{rc::Rc, sync::Arc};

use deno_core::{
  futures::{
    io::{BufReader, Cursor},
    FutureExt,
  },
  JsRuntime, ModuleLoadOptions, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode,
  ModuleSpecifier, ModuleType, PollEventLoopOptions, ResolutionKind, RuntimeOptions,
};
use deno_error::JsErrorBox;

struct EszipLoader(Arc<eszip::EszipV2>);

impl ModuleLoader for EszipLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    deno_core::resolve_import(specifier, referrer).map_err(|e| JsErrorBox::from_err(e))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&deno_core::ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let eszip = self.0.clone();
    let module_specifier = module_specifier.clone();

    ModuleLoadResponse::Async(
      async move {
        let module = eszip
          .get_module(module_specifier.as_str())
          .ok_or_else(|| JsErrorBox::generic(format!("Module not found: {}", module_specifier)))?;

        let code: Arc<[u8]> = module.source().await.ok_or_else(|| {
          JsErrorBox::generic(format!("Module source not found: {}", module_specifier))
        })?;

        let module_type = match module.kind {
          eszip::ModuleKind::JavaScript => ModuleType::JavaScript,
          eszip::ModuleKind::Json => ModuleType::Json,
          eszip::ModuleKind::Jsonc => ModuleType::Json,
          _ => ModuleType::JavaScript,
        };

        Ok(ModuleSource::new(
          module_type,
          ModuleSourceCode::Bytes(code.into()),
          &module_specifier,
          None,
        ))
      }
      .boxed_local(),
    )
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  deno_core::JsRuntime::init_platform(None);

  let bytes = std::fs::read("module.eszip")?;
  let reader = BufReader::new(Cursor::new(bytes));
  let (eszip, loader_fut) = eszip::EszipV2::parse(reader).await?;

  tokio::spawn(loader_fut);

  let eszip_arc = Arc::new(eszip);

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(EszipLoader(eszip_arc.clone()))),
    ..Default::default()
  });

  let main_module = deno_core::resolve_url("file:///main.js")?;
  let mod_id = runtime.load_main_es_module(&main_module).await?;
  let result = runtime.mod_evaluate(mod_id);

  runtime
    .run_event_loop(PollEventLoopOptions::default())
    .await?;
  result.await?;
  Ok(())
}
