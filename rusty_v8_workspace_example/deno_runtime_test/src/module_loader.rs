use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use deno_ast::{MediaType, ModuleSpecifier, ParseParams, SourceMapOption};
use deno_core::{
  error::ModuleLoaderError, resolve_import, ModuleLoadResponse, ModuleLoader, ModuleSource,
  ModuleSourceCode, ModuleType, RequestedModuleType, ResolutionKind,
};

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct TypescriptModuleLoader {
  pub source_maps: SourceMapStore,
}

impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    resolve_import(specifier, referrer).map_err(|e| deno_error::JsErrorBox::from_err(e))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    _requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    let source_maps = self.source_maps.clone();
    fn load(
      source_maps: SourceMapStore,
      module_specifier: &ModuleSpecifier,
    ) -> Result<ModuleSource, ModuleLoaderError> {
      println!("👀 load: {}", module_specifier);

      let (code, should_transpile, media_type, module_type) = if module_specifier.scheme() == "file"
      {
        let path = module_specifier.to_file_path().map_err(|_| {
          deno_error::JsErrorBox::generic(
            "There was an error converting the module specifier to a file path",
          )
        })?;

        let media_type = MediaType::from_path(&path);
        let (module_type, should_transpile) = match MediaType::from_path(&path) {
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
          _ => {
            return Err(deno_error::JsErrorBox::generic(format!(
              "Unknown extension {:?}",
              path.extension()
            )))
          }
        };

        (
          std::fs::read_to_string(&path)
            .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?,
          should_transpile,
          media_type,
          module_type,
        )
      } else if module_specifier.scheme() == "https" {
        let url = module_specifier.to_string();

        let response_text = ureq::get(&url)
          .call()
          .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?
          .into_body()
          .read_to_string()
          .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;

        (
          response_text,
          false,
          MediaType::JavaScript,
          ModuleType::JavaScript,
        )
      } else {
        println!("👀 unknown scheme {:?}", module_specifier.scheme());
        return Err(deno_error::JsErrorBox::generic(format!(
          "Unknown scheme {:?}",
          module_specifier.scheme()
        )));
      };

      let code = if should_transpile {
        let parsed = deno_ast::parse_module(ParseParams {
          specifier: module_specifier.clone(),
          text: code.into(),
          media_type,
          capture_tokens: false,
          scope_analysis: false,
          maybe_syntax: None,
        })
        .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
        let res = parsed
          .transpile(
            &deno_ast::TranspileOptions {
              imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
              use_decorators_proposal: true,
              ..Default::default()
            },
            &deno_ast::TranspileModuleOptions::default(),
            &deno_ast::EmitOptions {
              source_map: SourceMapOption::Separate,
              inline_sources: true,
              ..Default::default()
            },
          )
          .map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?;
        let res = res.into_source();
        let source_map = res.source_map.unwrap();
        source_maps
          .borrow_mut()
          .insert(module_specifier.to_string(), source_map.into_bytes());
        String::from_utf8(res.text.into_bytes()).unwrap()
      } else {
        code
      };
      Ok(ModuleSource::new(
        module_type,
        ModuleSourceCode::String(code.into()),
        module_specifier,
        None,
      ))
    }

    ModuleLoadResponse::Sync(load(source_maps, module_specifier))
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|v| Cow::Owned(v.clone()))
  }
}
