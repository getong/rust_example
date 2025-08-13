use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use deno_ast::{MediaType, ModuleSpecifier, ParseParams, SourceMapOption};
use deno_runtime::{
  deno_core::{
    ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleType,
    RequestedModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import, url::Url,
  },
  deno_fs::FileSystem,
};

use crate::{
  npm_downloader::{NpmConfig, NpmDownloader},
  npm_specifier::parse_npm_specifier,
};

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

pub struct TypescriptModuleLoader {
  pub source_maps: SourceMapStore,
  pub fs: Arc<dyn FileSystem>,
  pub npm_downloader: Arc<NpmDownloader>,
}

impl TypescriptModuleLoader {
  pub fn new(fs: Arc<dyn FileSystem>) -> Self {
    let npm_config = NpmConfig::default();
    let npm_downloader =
      Arc::new(NpmDownloader::new(npm_config).expect("Failed to create npm downloader"));

    Self {
      fs,
      source_maps: Rc::new(RefCell::new(HashMap::new())),
      npm_downloader,
    }
  }

  pub async fn resolve_and_ensure_npm_module(
    &self,
    specifier: &str,
    _referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    // Download the package if not already cached
    let cached_package = self
      .npm_downloader
      .download_package(specifier)
      .await
      .map_err(|e| {
        ModuleLoaderError::generic(format!(
          "Failed to download npm package {}: {}",
          specifier, e
        ))
      })?;

    let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
    let (_, _, sub_path) = parse_npm_specifier(package_name);

    // Resolve the main entry point or subpath
    let file_path = if let Some(sub_path) = sub_path {
      cached_package.path.join("package").join(sub_path)
    } else {
      // Use the main entry point from cached package or default to index.js
      if let Some(main_path) = self
        .npm_downloader
        .cache
        .get_main_entry_path(&cached_package)
      {
        main_path
      } else {
        cached_package.path.join("package").join("index.js")
      }
    };

    // Convert to file URL
    let file_url = Url::from_file_path(&file_path)
      .map_err(|_| ModuleLoaderError::generic("Failed to convert path to URL"))?;

    Ok(ModuleSpecifier::from(file_url))
  }
}

// Helper function to extract npm: imports from module content
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
  source_maps: SourceMapStore,
  _fs: Arc<dyn FileSystem>,
  downloader: &Arc<NpmDownloader>,
) -> Result<ModuleSource, ModuleLoaderError> {
  let path = module_specifier
    .to_file_path()
    .map_err(|_| ModuleLoaderError::generic("Only file:// URLs are supported"))?;

  let media_type = MediaType::from_path(&path);
  let (module_type, should_transpile) = match media_type {
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
    _ => {
      return Err(ModuleLoaderError::generic(format!(
        "Unknown extension {:?}",
        path.extension()
      )));
    }
  };

  // Read file
  let mut code = std::fs::read_to_string(&path)
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to read file: {}", e)))?;

  // Transform npm: imports to file:// URLs
  code = transform_npm_imports(code, downloader).await?;

  let code = if should_transpile {
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|e| ModuleLoaderError::generic(format!("Failed to parse module: {}", e)))?;

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
      .map_err(|e| ModuleLoaderError::generic(format!("Failed to transpile: {}", e)))?;

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
    &module_specifier,
    None,
  ))
}

async fn transform_npm_imports(
  mut code: String,
  downloader: &Arc<NpmDownloader>,
) -> Result<String, ModuleLoaderError> {
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

fn load(
  source_maps: SourceMapStore,
  module_specifier: &ModuleSpecifier,
) -> Result<ModuleSource, ModuleLoaderError> {
  println!("👀 load: {}", module_specifier);

  let (code, should_transpile, media_type, module_type) = if module_specifier.scheme() == "file" {
    let path = module_specifier.to_file_path().map_err(|_| {
      deno_error::JsErrorBox::generic(
        "There was an error converting the module specifier to a file path",
      )
    })?;

    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = match MediaType::from_path(&path) {
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
      _ => {
        return Err(deno_error::JsErrorBox::generic(format!(
          "Unknown extension {:?}",
          path.extension()
        )));
      }
    };

    (
      std::fs::read_to_string(&path).map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))?,
      should_transpile,
      media_type,
      module_type,
    )
  } else if module_specifier.scheme() == "https" {
    let url = module_specifier.to_string();

    // Try to use system proxy settings if available
    let mut builder = ureq::Agent::config_builder();

    // Check for proxy environment variables
    if let Ok(https_proxy) = std::env::var("https_proxy") {
      eprintln!("Using https_proxy: {}", https_proxy);
      if let Ok(proxy_url) = ureq::Proxy::new(&https_proxy) {
        builder = builder.proxy(Some(proxy_url));
      }
    } else if let Ok(http_proxy) = std::env::var("http_proxy") {
      eprintln!("Using http_proxy: {}", http_proxy);
      if let Ok(proxy_url) = ureq::Proxy::new(&http_proxy) {
        builder = builder.proxy(Some(proxy_url));
      }
    }

    let agent = ureq::Agent::new_with_config(builder.build());

    let response = agent
      .get(&url)
      .header("User-Agent", "Deno")
      .call()
      .map_err(|e| {
        eprintln!("Failed to fetch module from {}: {}", url, e);
        deno_error::JsErrorBox::generic(format!(
          "Failed to fetch module from {}: {}. Check your internet connection and firewall \
           settings.",
          url, e
        ))
      })?;

    let response_text = response.into_body().read_to_string().map_err(|e| {
      eprintln!("Failed to read response body from {}: {:?}", url, e);
      deno_error::JsErrorBox::generic(format!("Failed to read response body: {}", e))
    })?;

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
    module_specifier,
    None,
  ))
}

impl ModuleLoader for TypescriptModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    if specifier.starts_with("npm:") {
      // For npm: specifiers, try to resolve synchronously first (if already cached)
      let package_name = specifier.strip_prefix("npm:").unwrap_or(specifier);
      let (name, version, sub_path) = parse_npm_specifier(package_name);

      // Check if package is already cached (try both the specified version and "latest")
      let versions_to_check = if version == "latest" {
        // If looking for "latest", check all cached versions of this package
        if let Ok(packages) = self.npm_downloader.cache.list_packages() {
          let mut found_versions: Vec<String> = packages
            .iter()
            .filter(|p| p.name == name)
            .map(|p| p.version.clone())
            .collect();
          found_versions.sort();
          found_versions.reverse(); // Get the highest version first
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

          let file_url = Url::from_file_path(&file_path).map_err(|_| {
            ModuleLoaderError::generic("Failed to convert cached package path to URL")
          })?;

          return Ok(ModuleSpecifier::from(file_url));
        }
      }

      // Package not cached, return a placeholder URL that we'll resolve in load()
      let npm_url = format!("npm-resolve:{}", specifier);
      let result = ModuleSpecifier::parse(&npm_url).map_err(|e| {
        ModuleLoaderError::generic(format!("Failed to create npm placeholder URL: {}", e))
      })?;
      Ok(result)
    } else {
      resolve_import(specifier, referrer).map_err(|e| deno_error::JsErrorBox::from_err(e))
    }
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    _requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let source_maps = self.source_maps.clone();
    let fs = self.fs.clone();

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
            ModuleLoaderError::generic(format!(
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
          .map_err(|_| ModuleLoaderError::generic("Failed to convert path to URL"))?;
        let actual_module_specifier = ModuleSpecifier::from(actual_specifier);

        load(source_maps, &actual_module_specifier)
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
                return Err(ModuleLoaderError::generic(format!(
                  "Failed to download npm package {}: {}",
                  npm_import, e
                )));
              }
            }

            // Now load the module with resolved npm imports
            load_module_with_npm_resolution(module_spec_clone, source_maps, fs, &downloader).await
          }));
        }
      }
    }

    // Regular module loading (sync)
    ModuleLoadResponse::Sync(load(source_maps, &module_specifier))
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|v| Cow::Owned(v.clone()))
  }
}

/// Check if a JavaScript file is using CommonJS patterns
fn is_commonjs_module(code: &str) -> bool {
  // Look for CommonJS patterns
  code.contains("module.exports") ||
  code.contains("exports.") ||
  code.contains("exports[") ||
  // Also check if it doesn't have ES module exports
  (!code.contains("export ") && !code.contains("export{") && !code.contains("export*"))
}

/// Wrap CommonJS module to make it ES module compatible
fn wrap_commonjs_module(code: String) -> String {
  format!(
    r#"
// CommonJS to ES Module wrapper
const {{ module, exports }} = globalThis.__createCommonJSContext();
{}
export default module.exports;
export {{ module as __module, exports as __exports }};
"#,
    code
  )
}
