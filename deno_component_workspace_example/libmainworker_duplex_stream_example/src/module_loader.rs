use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use deno_ast::{MediaType, ParseParams, SourceMapOption};
use deno_core::{
  ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
  ModuleSourceCode, ModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import,
};
use deno_error::JsErrorBox;

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

const SHIM_MODULES_SOURCE: &str = include_str!("shim_modules.ts");

pub(crate) struct DirectModuleLoader {
  source_maps: SourceMapStore,
}

impl DirectModuleLoader {
  pub(crate) fn new() -> Self {
    Self {
      source_maps: Rc::new(RefCell::new(HashMap::new())),
    }
  }

  fn module_kind(media_type: MediaType) -> Result<(ModuleType, bool), ModuleLoaderError> {
    match media_type {
      MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
        Ok((ModuleType::JavaScript, false))
      }
      MediaType::Jsx => Ok((ModuleType::JavaScript, true)),
      MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => Ok((ModuleType::JavaScript, true)),
      MediaType::Json => Ok((ModuleType::Json, false)),
      MediaType::Unknown => Ok((ModuleType::JavaScript, false)),
      _ => Err(JsErrorBox::generic(format!(
        "unsupported media type: {media_type:?}"
      ))),
    }
  }

  fn transpile_if_needed(
    source_maps: SourceMapStore,
    module_specifier: &deno_core::ModuleSpecifier,
    code: String,
    media_type: MediaType,
    should_transpile: bool,
  ) -> Result<String, ModuleLoaderError> {
    if !should_transpile {
      return Ok(code);
    }

    let parsed = deno_ast::parse_module(ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;

    let transpiled = parsed
      .transpile(
        &deno_ast::TranspileOptions {
          imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
          decorators: deno_ast::DecoratorsTranspileOption::Ecma,
          ..Default::default()
        },
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions {
          source_map: SourceMapOption::Separate,
          inline_sources: true,
          ..Default::default()
        },
      )
      .map_err(|err| JsErrorBox::generic(err.to_string()))?
      .into_source();

    if let Some(source_map) = transpiled.source_map {
      source_maps
        .borrow_mut()
        .insert(module_specifier.to_string(), source_map.into_bytes());
    }

    String::from_utf8(transpiled.text.into_bytes())
      .map_err(|err| JsErrorBox::generic(err.to_string()))
  }

  fn load_file_module(
    source_maps: SourceMapStore,
    module_specifier: &deno_core::ModuleSpecifier,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let path = module_specifier.to_file_path().map_err(|_| {
      JsErrorBox::generic("there was an error converting the module specifier to a file path")
    })?;
    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = Self::module_kind(media_type)?;
    let code =
      std::fs::read_to_string(&path).map_err(|err| JsErrorBox::generic(err.to_string()))?;
    let code = Self::transpile_if_needed(
      source_maps,
      module_specifier,
      code,
      media_type,
      should_transpile,
    )?;

    Ok(ModuleSource::new(
      module_type,
      ModuleSourceCode::String(code.into()),
      module_specifier,
      None,
    ))
  }

  fn parse_npm_package_name(raw_specifier: &str) -> String {
    if raw_specifier.starts_with('@') {
      let mut it = raw_specifier.splitn(3, '/');
      let scope = it.next().unwrap_or_default();
      let name_and_rest = it.next().unwrap_or_default();
      let base_name = name_and_rest.split('@').next().unwrap_or(name_and_rest);
      format!("{scope}/{base_name}")
    } else {
      raw_specifier
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string()
    }
  }

  fn parse_jsr_package_name_and_subpath(raw_specifier: &str) -> (String, String) {
    if raw_specifier.starts_with('@') {
      let mut it = raw_specifier.splitn(3, '/');
      let scope = it.next().unwrap_or_default();
      let name_and_rest = it.next().unwrap_or_default();
      let base_name = name_and_rest.split('@').next().unwrap_or(name_and_rest);
      let package_name = format!("{scope}/{base_name}");
      let subpath = it.next().unwrap_or_default().to_string();
      (package_name, subpath)
    } else {
      let mut it = raw_specifier.splitn(2, '/');
      let package_name = it
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string();
      let subpath = it.next().unwrap_or_default().to_string();
      (package_name, subpath)
    }
  }

  fn load_shim_module(
    module_specifier: &deno_core::ModuleSpecifier,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let spec = module_specifier.as_str();
    if let Some(raw) = spec.strip_prefix("npm:") {
      match Self::parse_npm_package_name(raw).as_str() {
        "nanoid" | "date-fns" | "lodash-es" | "zod" | "stream-chat" => {}
        pkg => {
          return Err(JsErrorBox::generic(format!(
            "unsupported npm package in direct mode: {pkg}"
          )));
        }
      }
    } else if let Some(raw) = spec.strip_prefix("jsr:") {
      let (package_name, subpath) = Self::parse_jsr_package_name_and_subpath(raw);
      match package_name.as_str() {
        "@std/dotenv" => {}
        "@std/async" => {
          if !(subpath.is_empty() || subpath == "delay") {
            return Err(JsErrorBox::generic(format!(
              "unsupported jsr subpath in direct mode: @std/async/{subpath}"
            )));
          }
        }
        pkg => {
          return Err(JsErrorBox::generic(format!(
            "unsupported jsr package in direct mode: {pkg}"
          )));
        }
      }
    } else {
      return Err(JsErrorBox::generic(format!(
        "unsupported module scheme: {}",
        module_specifier.scheme()
      )));
    }

    Ok(ModuleSource::new(
      ModuleType::JavaScript,
      ModuleSourceCode::String(SHIM_MODULES_SOURCE.to_string().into()),
      module_specifier,
      None,
    ))
  }
}

impl ModuleLoader for DirectModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    if specifier.starts_with("npm:") || specifier.starts_with("jsr:") {
      return deno_core::ModuleSpecifier::parse(specifier).map_err(JsErrorBox::from_err);
    }
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &deno_core::ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let result = match module_specifier.scheme() {
      "file" => Self::load_file_module(self.source_maps.clone(), module_specifier),
      "npm" | "jsr" => Self::load_shim_module(module_specifier),
      scheme => Err(JsErrorBox::generic(format!(
        "unsupported module scheme: {scheme}"
      ))),
    };

    ModuleLoadResponse::Sync(result)
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|source_map| Cow::Owned(source_map.clone()))
  }
}
