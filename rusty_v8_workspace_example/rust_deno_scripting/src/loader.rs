use std::env;

use anyhow::{Result, anyhow, bail};
use deno_ast::{MediaType, ParseParams};
use deno_core::{
  ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
  RequestedModuleType, ResolutionKind, resolve_import, resolve_path,
};
use deno_error::JsErrorBox;

// By implementing a custom module loader, we can change where imported modules are loaded from
// or transpile them from other languages (such as TypeScript) to JavaScript,
// We can also alter the behavior depending on whether the import is a dynamic import or an
// import statement, or depending on where the import was triggered from.
// To ensure the Deno language server understands what code these imports map to,
// we also provide a "deno.json" at the root of this repository.
// When exposing an API for your script developers to target, you could also provide a
// "deno.json" that redirects your provided modules to type definition files on the web
// without exposing the internal implementations.
pub struct TypescriptModuleLoader;

// When implementing our own module loader, we can introduce special handling
// for protocol schemes as well as the whole module specifier.
// For demonstration purposes, this module loader implements a custom protocol scheme
// called "builtin:" that returns certain modules directly bundled into our program's
// binary (using `include_str!`), as well as a prefix matcher on the module identifier
// of normal protocol-less imports (resulting in the default scheme "file:") that rewrites
// the module prefix to a specific directory on our disk.
const INTERNAL_MODULE_PREFIX: &str = "@builtin/";

impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> std::result::Result<ModuleSpecifier, JsErrorBox> {
    if specifier.starts_with(INTERNAL_MODULE_PREFIX) {
      let mut path_str = specifier.replace(INTERNAL_MODULE_PREFIX, "./builtins/");
      // For module specifiers starting with our "builtin" prefix, we automatically add the ".ts"
      // extension.
      // By default, Deno requires specifying the full file name including file extensions.
      path_str.push_str(".ts");
      return resolve_path(
        &path_str,
        &env::current_dir().map_err(|e| JsErrorBox::generic(e.to_string()))?,
      )
      .map_err(|e| JsErrorBox::generic(e.to_string()));
    }
    resolve_import(specifier, referrer).map_err(|e| JsErrorBox::generic(e.to_string()))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&deno_core::url::Url>,
    is_dyn_import: bool,
    requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    // We only make use of synchronous sources for our modules in this demo,
    // but you may also return an async response to, for example, fetch files
    // from the network.
    ModuleLoadResponse::Sync(
      self
        .load_sync(
          module_specifier,
          maybe_referrer,
          is_dyn_import,
          requested_module_type,
        )
        .map_err(|e| JsErrorBox::generic(e.to_string())),
    )
  }
}

impl TypescriptModuleLoader {
  fn load_sync(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&deno_core::url::Url>,
    _is_dyn_import: bool,
    requested_module_type: RequestedModuleType,
  ) -> Result<ModuleSource> {
    let module_code = if module_specifier.scheme() == "builtin" {
      ModuleCode::from_builtin(&module_specifier)?
    } else {
      ModuleCode::from_file(&module_specifier, requested_module_type)?
    };

    // TypeScript files must first be transpiled to JavaScript before they can be executed.
    // For this purpose, Deno wraps https://swc.rs and exposes it as part of the `deno_ast` module.
    let code = ModuleSourceCode::String(
      if module_code.should_transpile {
        let parsed_source = deno_ast::parse_module(ParseParams {
          specifier: module_specifier.clone(),
          text: module_code.code.into(),
          media_type: module_code.media_type,
          capture_tokens: false,
          scope_analysis: false,
          maybe_syntax: None,
        })?;
        let transpiled_source = parsed_source
          .transpile(
            &Default::default(),
            &Default::default(),
            &Default::default(),
          )?
          .into_source();
        transpiled_source.text.to_string()
      } else {
        module_code.code
      }
      .into(),
    );
    let module = ModuleSource::new(module_code.module_type, code, &module_specifier, None);
    Ok(module)
  }
}

struct ModuleCode {
  media_type: MediaType,
  module_type: ModuleType,
  should_transpile: bool,
  code: String,
}

impl ModuleCode {
  fn from_builtin(module_specifier: &ModuleSpecifier) -> Result<Self> {
    // As stated above, we map the "builtin:" protocol to a premade set of
    // internal modules bundled into our binary.
    // The "state.ts" module simply exposes a more convenient API for our
    // internal ops.
    let code = match module_specifier.path() {
      "state" => include_str!("../builtins/state.ts"),
      _ => bail!("no builtin module {module_specifier}"),
    };

    Ok(Self {
      media_type: MediaType::Mts,
      module_type: ModuleType::JavaScript,
      should_transpile: true,
      code: code.to_string(),
    })
  }

  fn from_file(
    module_specifier: &ModuleSpecifier,
    requested_module_type: RequestedModuleType,
  ) -> Result<Self> {
    // We only implement a synchronous module loader here that reads files from disk,
    // but we could also implement asynchronous loaders that fetch files from the network
    // using http or any other custom protocol identifier we desire.
    let path = module_specifier
      .to_file_path()
      .map_err(|_| anyhow!("Only file: URLs are supported."))?;

    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = match requested_module_type {
      RequestedModuleType::None => match media_type {
        MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => (ModuleType::JavaScript, false),
        MediaType::Jsx => (ModuleType::JavaScript, true),
        MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => (ModuleType::JavaScript, true),
        MediaType::Json => (ModuleType::Json, false),
        _ => bail!("Unknown file extension {:?}", path.extension()),
      },
      RequestedModuleType::Json => (ModuleType::Json, false),
      RequestedModuleType::Other(module_type) => bail!("Unknown module type {}", module_type),
      RequestedModuleType::Text => bail!("Text module type not supported"),
      RequestedModuleType::Bytes => bail!("Bytes module type not supported"),
    };

    let code = std::fs::read_to_string(&path)?;

    Ok(Self {
      media_type,
      module_type,
      should_transpile,
      code,
    })
  }
}
