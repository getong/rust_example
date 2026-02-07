use std::{cell::RefCell, collections::HashMap, env, rc::Rc, sync::Arc};

use anyhow::{Result, anyhow, bail};
use deno_ast::{MediaType, ParseParams};
use deno_core::{
  ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
  ModuleSourceCode, ModuleSpecifier, ModuleType, RequestedModuleType, ResolutionKind,
  resolve_import, resolve_path, url::Url,
};
use deno_error::JsErrorBox;

use crate::{
  npm_downloader::{NpmConfig, NpmDownloader},
  npm_specifier::parse_npm_specifier,
};

const INTERNAL_MODULE_PREFIX: &str = "@builtin/";

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct TypescriptModuleLoader {
  source_maps: SourceMapStore,
  npm_downloader: Arc<NpmDownloader>,
}

impl TypescriptModuleLoader {
  pub fn new() -> Self {
    let npm_config = NpmConfig::default();
    let npm_downloader =
      Arc::new(NpmDownloader::new(npm_config).expect("Failed to create npm downloader"));

    Self {
      source_maps: Rc::new(RefCell::new(HashMap::new())),
      npm_downloader,
    }
  }
}

impl Default for TypescriptModuleLoader {
  fn default() -> Self {
    Self::new()
  }
}

impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> std::result::Result<ModuleSpecifier, JsErrorBox> {
    if specifier.starts_with(INTERNAL_MODULE_PREFIX) {
      let mut path_str = specifier.replace(INTERNAL_MODULE_PREFIX, "./builtins/");
      path_str.push_str(".ts");
      return resolve_path(
        &path_str,
        &env::current_dir().map_err(|e| JsErrorBox::generic(e.to_string()))?,
      )
      .map_err(|e| JsErrorBox::generic(e.to_string()));
    }

    if specifier.starts_with("npm:") {
      // For npm: specifiers, try to resolve synchronously first (if already cached)
      let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
      let (name, version, sub_path) = parse_npm_specifier(package_name);

      // Check if package is already cached
      let versions_to_check = if version == "latest" {
        if let Ok(packages) = self.npm_downloader.cache.list_packages() {
          let mut found_versions: Vec<String> = packages
            .iter()
            .filter(|p| p.name == name)
            .map(|p| p.version.clone())
            .collect();
          found_versions.sort();
          found_versions.reverse();
          found_versions
        } else {
          vec!["latest".to_string()]
        }
      } else {
        vec![version.clone()]
      };

      for check_version in versions_to_check {
        if let Ok(Some(cached_package)) =
          self.npm_downloader.cache.get_package(&name, &check_version)
        {
          // Package is cached, resolve to actual file path
          let file_path = if let Some(sub_path) = sub_path.clone() {
            cached_package.path.join("package").join(sub_path)
          } else if let Some(main_path) = self
            .npm_downloader
            .cache
            .get_main_entry_path(&cached_package)
          {
            main_path
          } else {
            cached_package.path.join("package").join("index.js")
          };

          let file_url = Url::from_file_path(&file_path)
            .map_err(|_| JsErrorBox::generic("Failed to convert cached package path to URL"))?;

          return Ok(ModuleSpecifier::from(file_url));
        }
      }

      // Package not cached, return a placeholder URL that we'll resolve in load()
      let npm_url = format!("npm-resolve:{}", specifier);
      let result = ModuleSpecifier::parse(&npm_url)
        .map_err(|e| JsErrorBox::generic(format!("Failed to create npm placeholder URL: {}", e)))?;
      Ok(result)
    } else {
      resolve_import(specifier, referrer).map_err(|e| JsErrorBox::generic(e.to_string()))
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let source_maps = self.source_maps.clone();
    let requested_module_type = options.requested_module_type;

    // Check if this is an npm-resolve: specifier that needs async resolution
    if module_specifier.scheme() == "npm-resolve" {
      let npm_specifier = module_specifier
        .as_str()
        .strip_prefix("npm-resolve:")
        .unwrap_or("")
        .to_string();
      let downloader = self.npm_downloader.clone();

      return ModuleLoadResponse::Async(Box::pin(async move {
        println!(
          "📥 Downloading npm package with dependencies: {}",
          npm_specifier
        );

        // Download and resolve the npm package with all its dependencies
        let cached_package = downloader
          .download_package_with_dependencies(&npm_specifier)
          .await
          .map_err(|e| {
            JsErrorBox::generic(format!(
              "Failed to download npm package {}: {}",
              npm_specifier, e
            ))
          })?;

        println!(
          "✅ Successfully downloaded and cached: {} v{}",
          cached_package.name, cached_package.version
        );

        let package_name = npm_specifier.strip_prefix("npm:").unwrap_or(&npm_specifier);
        let (_, _, sub_path) = parse_npm_specifier(package_name);

        // Resolve the main entry point or subpath
        let file_path = if let Some(sub_path) = sub_path {
          cached_package.path.join("package").join(sub_path)
        } else {
          // Use the main entry point from cached package or default to index.js
          if let Some(main_path) = downloader.cache.get_main_entry_path(&cached_package) {
            main_path
          } else {
            cached_package.path.join("package").join("index.js")
          }
        };

        // Now load the actual file
        let actual_specifier = Url::from_file_path(&file_path)
          .map_err(|_| JsErrorBox::generic("Failed to convert path to URL"))?;
        let actual_module_specifier = ModuleSpecifier::from(actual_specifier);

        load_module(actual_module_specifier, source_maps)
      }));
    }

    // Check if this is a regular file that might contain npm: imports
    if let Ok(path) = module_specifier.to_file_path() {
      if let Ok(content) = std::fs::read_to_string(&path) {
        let npm_imports = extract_npm_imports(&content);

        if !npm_imports.is_empty() {
          // We found npm imports, handle them asynchronously
          let downloader = self.npm_downloader.clone();
          let module_spec_clone = module_specifier.clone();

          return ModuleLoadResponse::Async(Box::pin(async move {
            // First, download all npm dependencies with their dependencies
            for npm_import in npm_imports {
              if let Err(e) = downloader
                .download_package_with_dependencies(&npm_import)
                .await
              {
                return Err(JsErrorBox::generic(format!(
                  "Failed to download npm package {}: {}",
                  npm_import, e
                )));
              }
            }

            // Now load the module with resolved npm imports
            load_module_with_npm_resolution(module_spec_clone, source_maps, &downloader).await
          }));
        }
      }
    }

    // Regular module loading (sync)
    ModuleLoadResponse::Sync(
      self
        .load_sync(&module_specifier, requested_module_type)
        .map_err(|e| JsErrorBox::generic(e.to_string())),
    )
  }
}

impl TypescriptModuleLoader {
  fn load_sync(
    &self,
    module_specifier: &ModuleSpecifier,
    requested_module_type: RequestedModuleType,
  ) -> Result<ModuleSource> {
    let module_code = if module_specifier.scheme() == "builtin" {
      ModuleCode::from_builtin(&module_specifier)?
    } else {
      ModuleCode::from_file(&module_specifier, requested_module_type)?
    };

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

fn extract_npm_imports(content: &str) -> Vec<String> {
  let mut imports = Vec::new();

  // Simple regex-like parsing for npm: imports
  for line in content.lines() {
    if let Some(import_start) = line.find("from \"npm:") {
      let after_npm = &line[import_start + 10 ..];
      if let Some(quote_end) = after_npm.find('"') {
        let npm_spec = format!("npm:{}", &after_npm[.. quote_end]);
        imports.push(npm_spec);
      }
    } else if let Some(import_start) = line.find("from 'npm:") {
      let after_npm = &line[import_start + 10 ..];
      if let Some(quote_end) = after_npm.find('\'') {
        let npm_spec = format!("npm:{}", &after_npm[.. quote_end]);
        imports.push(npm_spec);
      }
    }
  }

  imports
}

async fn load_module_with_npm_resolution(
  module_specifier: ModuleSpecifier,
  _source_maps: SourceMapStore,
  downloader: &Arc<NpmDownloader>,
) -> Result<ModuleSource, JsErrorBox> {
  let path = module_specifier
    .to_file_path()
    .map_err(|_| JsErrorBox::generic("Only file:// URLs are supported"))?;

  let media_type = deno_graph::MediaType::from_specifier(&module_specifier);
  let (module_type, should_transpile) = match media_type {
    deno_graph::MediaType::JavaScript | deno_graph::MediaType::Mjs | deno_graph::MediaType::Cjs => {
      (ModuleType::JavaScript, false)
    }
    deno_graph::MediaType::Jsx => (ModuleType::JavaScript, true),
    deno_graph::MediaType::TypeScript
    | deno_graph::MediaType::Mts
    | deno_graph::MediaType::Cts
    | deno_graph::MediaType::Dts
    | deno_graph::MediaType::Dmts
    | deno_graph::MediaType::Dcts
    | deno_graph::MediaType::Tsx => (ModuleType::JavaScript, true),
    deno_graph::MediaType::Json => (ModuleType::Json, false),
    _ => {
      return Err(JsErrorBox::generic(format!(
        "Unknown extension {:?}",
        path.extension()
      )));
    }
  };

  // Read file
  let mut code = std::fs::read_to_string(&path)
    .map_err(|e| JsErrorBox::generic(format!("Failed to read file: {}", e)))?;

  // Transform npm: imports to file:// URLs
  code = transform_npm_imports(code, downloader).await?;

  let code = if should_transpile {
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|e| JsErrorBox::generic(format!("Failed to parse module: {}", e)))?;

    let transpiled = parsed
      .transpile(
        &deno_ast::TranspileOptions::default(),
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions::default(),
      )
      .map_err(|e| JsErrorBox::generic(format!("Failed to transpile: {}", e)))?;

    transpiled.into_source().text
  } else {
    code
  };

  Ok(ModuleSource::new(
    module_type,
    ModuleSourceCode::String(code.into()),
    &module_specifier,
    None,
  ))
}

async fn transform_npm_imports(
  mut code: String,
  downloader: &Arc<NpmDownloader>,
) -> Result<String, JsErrorBox> {
  let npm_imports = extract_npm_imports(&code);

  for npm_import in npm_imports {
    // Get the cached package (should already be downloaded)
    let package_name = npm_import.strip_prefix("npm:").unwrap_or(&npm_import);
    let (name, version, sub_path) = parse_npm_specifier(package_name);

    // Resolve version if needed
    let resolved_version = if version == "latest" {
      // Would need to resolve latest version, for now use "latest"
      "latest".to_string()
    } else {
      version
    };

    if let Ok(Some(cached_package)) = downloader.cache.get_package(&name, &resolved_version) {
      let file_path = if let Some(sub_path) = sub_path {
        cached_package.path.join("package").join(sub_path)
      } else if let Some(main_path) = downloader.cache.get_main_entry_path(&cached_package) {
        main_path
      } else {
        cached_package.path.join("package").join("index.js")
      };

      if let Ok(file_url) = Url::from_file_path(&file_path) {
        let file_url_str = file_url.to_string();

        // Replace npm: import with file:// URL
        code = code.replace(
          &format!("\"{}\"", npm_import),
          &format!("\"{}\"", file_url_str),
        );
        code = code.replace(&format!("'{}'", npm_import), &format!("'{}'", file_url_str));
      }
    }
  }

  Ok(code)
}

fn load_module(
  module_specifier: ModuleSpecifier,
  _source_maps: SourceMapStore,
) -> Result<ModuleSource, JsErrorBox> {
  let path = module_specifier
    .to_file_path()
    .map_err(|_| JsErrorBox::generic("Only file:// URLs are supported"))?;

  let media_type = deno_graph::MediaType::from_specifier(&module_specifier);
  let (module_type, should_transpile) = match media_type {
    deno_graph::MediaType::JavaScript | deno_graph::MediaType::Mjs | deno_graph::MediaType::Cjs => {
      (ModuleType::JavaScript, false)
    }
    deno_graph::MediaType::Jsx => (ModuleType::JavaScript, true),
    deno_graph::MediaType::TypeScript
    | deno_graph::MediaType::Mts
    | deno_graph::MediaType::Cts
    | deno_graph::MediaType::Dts
    | deno_graph::MediaType::Dmts
    | deno_graph::MediaType::Dcts
    | deno_graph::MediaType::Tsx => (ModuleType::JavaScript, true),
    deno_graph::MediaType::Json => (ModuleType::Json, false),
    _ => {
      return Err(JsErrorBox::generic(format!(
        "Unknown extension {:?}",
        path.extension()
      )));
    }
  };

  // Read file synchronously using std::fs since we're in sync context
  let code = std::fs::read_to_string(&path)
    .map_err(|e| JsErrorBox::generic(format!("Failed to read file: {}", e)))?;

  let code = if should_transpile {
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|e| JsErrorBox::generic(format!("Failed to parse module: {}", e)))?;

    let transpiled = parsed
      .transpile(
        &deno_ast::TranspileOptions::default(),
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions::default(),
      )
      .map_err(|e| JsErrorBox::generic(format!("Failed to transpile: {}", e)))?;

    transpiled.into_source().text
  } else {
    // Check if this is a CommonJS module that needs wrapping
    if is_commonjs_module(&code) {
      wrap_commonjs_module(code)
    } else {
      code
    }
  };

  Ok(ModuleSource::new(
    module_type,
    ModuleSourceCode::String(code.into()),
    &module_specifier,
    None,
  ))
}

fn is_commonjs_module(code: &str) -> bool {
  code.contains("module.exports")
    || code.contains("exports.")
    || code.contains("exports[")
    || (!code.contains("export ") && !code.contains("export{") && !code.contains("export*"))
}

fn wrap_commonjs_module(code: String) -> String {
  format!(
    r#"
const {{ module, exports }} = globalThis.__createCommonJSContext();
{}
export default module.exports;
export {{ module as __module, exports as __exports }};
"#,
    code
  )
}

struct ModuleCode {
  media_type: MediaType,
  module_type: ModuleType,
  should_transpile: bool,
  code: String,
}

impl ModuleCode {
  fn from_builtin(module_specifier: &ModuleSpecifier) -> Result<Self> {
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
