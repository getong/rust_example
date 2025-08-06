use deno_ast::{MediaType, ParseParams, TranspileOptions};
use deno_core::{
  FastString, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier,
  ModuleType, RequestedModuleType, ResolutionKind,
  anyhow::{anyhow, bail},
  futures::{FutureExt, TryFutureExt},
  resolve_import,
};
use deno_error::JsErrorBox;

pub struct TypescriptModuleLoader {
  http: reqwest::Client,
}

impl Default for TypescriptModuleLoader {
  fn default() -> Self {
    Self {
      http: reqwest::Client::new(),
    }
  }
}

impl TypescriptModuleLoader {
  pub fn new(http: reqwest::Client) -> Self {
    Self { http }
  }
}

impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    resolve_import(specifier, referrer).map_err(|e| JsErrorBox::type_error(e.to_string()))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let http = self.http.clone();
    let future = async move {
      let (code, module_type, media_type, should_transpile) = match module_specifier.to_file_path()
      {
        Ok(path) => {
          let media_type = MediaType::from_path(&path);

          let (module_type, should_transpile) = match media_type {
            MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
              (ModuleType::JavaScript, false)
            }
            MediaType::Jsx => (ModuleType::JavaScript, true),
            MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Dts
            | MediaType::Dmts
            | MediaType::Dcts
            | MediaType::Tsx => (ModuleType::JavaScript, true),
            MediaType::Json => (ModuleType::Json, false),
            _ => bail!("Unknown extension {:?}", path.extension()),
          };

          if module_type == ModuleType::Json && requested_module_type != RequestedModuleType::Json {
            return Err(anyhow!(
              "Attempted to load JSON module without specifying \"type\": \"json\" attribute in \
               the import statement."
            ));
          }

          (
            tokio::fs::read_to_string(&path).await?,
            module_type,
            media_type,
            should_transpile,
          )
        }

        Err(_) => {
          if module_specifier.scheme() == "http" || module_specifier.scheme() == "https" {
            let http_res = http.get(module_specifier.to_string()).send().await?;

            if !http_res.status().is_success() {
              bail!("Failed to fetch module: {module_specifier}");
            }

            let content_type = http_res
              .headers()
              .get("content-type")
              .and_then(|ct| ct.to_str().ok())
              .ok_or_else(|| anyhow!("No content-type header"))?;

            let media_type = MediaType::from_content_type(&module_specifier, content_type);

            let (module_type, should_transpile) = match media_type {
              MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                (ModuleType::JavaScript, false)
              }
              MediaType::Jsx => (ModuleType::JavaScript, true),
              MediaType::TypeScript
              | MediaType::Mts
              | MediaType::Cts
              | MediaType::Dts
              | MediaType::Dmts
              | MediaType::Dcts
              | MediaType::Tsx => (ModuleType::JavaScript, true),
              MediaType::Json => (ModuleType::Json, false),
              _ => bail!("Unknown content-type {:?}", content_type),
            };

            if module_type == ModuleType::Json && requested_module_type != RequestedModuleType::Json
            {
              return Err(anyhow!(
                "Attempted to load JSON module without specifying \"type\": \"json\" attribute in \
                 the import statement."
              ));
            }

            let code = http_res.text().await?;

            (code, module_type, media_type, should_transpile)
          } else {
            bail!("Unsupported module specifier: {}", module_specifier);
          }
        }
      };

      let code = if should_transpile {
        let parsed = deno_ast::parse_module(ParseParams {
          specifier: module_specifier.clone(),
          text: code.into(),
          media_type,
          capture_tokens: false,
          scope_analysis: false,
          maybe_syntax: None,
        })?;

        let transpile_options = TranspileOptions::default();
        let transpiled =
          parsed.transpile(&transpile_options, &Default::default(), &Default::default())?;
        transpiled.into_source().text
      } else {
        code
      };

      let module = ModuleSource::new(
        module_type,
        ModuleSourceCode::String(FastString::from(code)),
        &module_specifier,
        None,
      );

      Ok(module)
    }
    .map_err(|e| JsErrorBox::type_error(e.to_string()))
    .boxed_local();

    ModuleLoadResponse::Async(future)
  }
}

// copy from https://github.com/CheatCod/basic_deno_ts_module_loader
// modify with claude code
// also see https://cheatcod3.hashnode.dev/embedding-typescript-in-your-rust-project
