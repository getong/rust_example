use std::{path::PathBuf, rc::Rc, sync::Arc};

use anyhow::Result;
use deno_core::{
  ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, RequestedModuleType,
  ResolutionKind, error::AnyError,
};
use deno_lib::{
  args::{CliOptions, Flags},
  factory::CliFactory,
};
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;
use indexmap::IndexMap;

pub struct CustomModuleLoader {
  fs: Arc<dyn FileSystem>,
}

impl CustomModuleLoader {
  pub fn new(fs: Arc<dyn FileSystem>) -> Self {
    Self { fs }
  }
}

#[async_trait::async_trait(?Send)]
impl ModuleLoader for CustomModuleLoader {
  async fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    if specifier.starts_with("npm:") {
      // For now, we'll create a placeholder resolution for npm specifiers
      // This would need full npm resolution implementation from deno_lib
      anyhow::bail!(
        "npm: specifiers require full deno_lib integration. Consider using the deno CLI directly \
         or implementing a full npm resolver."
      )
    } else {
      deno_core::resolve_import(specifier, referrer.as_str()).map_err(|e| e.into())
    }
  }

  async fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    _requested_module_type: RequestedModuleType,
  ) -> Result<ModuleSource, AnyError> {
    let path = module_specifier
      .to_file_path()
      .map_err(|_| anyhow::anyhow!("Only file:// URLs are supported"))?;

    let media_type = deno_graph::MediaType::from_specifier(module_specifier);
    let (module_type, should_transpile) = match media_type {
      deno_graph::MediaType::JavaScript
      | deno_graph::MediaType::Mjs
      | deno_graph::MediaType::Cjs => (ModuleType::JavaScript, false),
      deno_graph::MediaType::Jsx => (ModuleType::JavaScript, true),
      deno_graph::MediaType::TypeScript
      | deno_graph::MediaType::Mts
      | deno_graph::MediaType::Cts
      | deno_graph::MediaType::Dts
      | deno_graph::MediaType::Dmts
      | deno_graph::MediaType::Dcts
      | deno_graph::MediaType::Tsx => (ModuleType::JavaScript, true),
      deno_graph::MediaType::Json => (ModuleType::Json, false),
      _ => anyhow::bail!("Unknown extension {:?}", path.extension()),
    };

    let code = self
      .fs
      .read_file_sync(&path, None)
      .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;
    let code = String::from_utf8(code)?;

    let code = if should_transpile {
      let parsed = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: module_specifier.clone(),
        text: code.into(),
        media_type,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      })?;
      parsed
        .transpile(
          &deno_ast::TranspileOptions::default(),
          &deno_ast::EmitOptions::default(),
        )?
        .into_source()
        .into_string()?
        .text
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
}
