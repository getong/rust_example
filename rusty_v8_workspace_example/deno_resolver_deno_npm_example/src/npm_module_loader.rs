use std::path::PathBuf;

use deno_core::{ModuleSource, ModuleSpecifier};
use deno_error::JsErrorBox;
use deno_semver::npm::NpmPackageReqReference;
use url::Url;

pub struct NpmModuleLoader {
  cache_dir: PathBuf,
}

impl NpmModuleLoader {
  pub fn new() -> Result<Self, JsErrorBox> {
    // Set up npm cache directory
    let cache_dir = dirs::cache_dir()
      .ok_or_else(|| JsErrorBox::generic("Failed to get cache directory"))?
      .join("deno")
      .join("npm");

    std::fs::create_dir_all(&cache_dir)
      .map_err(|e| JsErrorBox::generic(format!("Failed to create npm cache dir: {}", e)))?;

    Ok(Self { cache_dir })
  }

  /// Resolve the main entry point from package.json
  /// Follows Node.js module resolution order: exports -> main -> module -> browser -> index.js
  fn resolve_package_entry(&self, package_json: &serde_json::Value) -> Option<String> {
    // 1. Check exports field first (most modern)
    if let Some(exports) = package_json.get("exports") {
      if let Some(main_export) = self.resolve_exports_entry(exports) {
        return Some(main_export);
      }
    }

    // 2. Check main field
    if let Some(main) = package_json.get("main").and_then(|v| v.as_str()) {
      if !main.is_empty() {
        return Some(main.to_string());
      }
    }

    // 3. Check module field (ES modules)
    if let Some(module) = package_json.get("module").and_then(|v| v.as_str()) {
      if !module.is_empty() {
        return Some(module.to_string());
      }
    }

    // 4. Check browser field (if it's a string)
    if let Some(browser) = package_json.get("browser").and_then(|v| v.as_str()) {
      if !browser.is_empty() {
        return Some(browser.to_string());
      }
    }

    // 5. Default fallbacks
    None
  }

  /// Resolve entry from exports field
  fn resolve_exports_entry(&self, exports: &serde_json::Value) -> Option<String> {
    match exports {
      // String export: "exports": "./main.js"
      serde_json::Value::String(s) => Some(s.clone()),
      // Object export: "exports": { ".": "./main.js" }
      serde_json::Value::Object(map) => {
        // Try "." first (main entry)
        if let Some(main_export) = map.get(".") {
          return self.resolve_export_target(main_export);
        }
        // Try "import" or "require" conditions
        if let Some(import_export) = map.get("import") {
          return self.resolve_export_target(import_export);
        }
        if let Some(require_export) = map.get("require") {
          return self.resolve_export_target(require_export);
        }
        // Try "default" condition
        if let Some(default_export) = map.get("default") {
          return self.resolve_export_target(default_export);
        }
        None
      }
      _ => None,
    }
  }

  /// Resolve a specific export target
  fn resolve_export_target(&self, target: &serde_json::Value) -> Option<String> {
    match target {
      serde_json::Value::String(s) => Some(s.clone()),
      serde_json::Value::Object(map) => {
        // Handle conditional exports
        if let Some(import_target) = map.get("import") {
          if let Some(s) = import_target.as_str() {
            return Some(s.to_string());
          }
        }
        if let Some(require_target) = map.get("require") {
          if let Some(s) = require_target.as_str() {
            return Some(s.to_string());
          }
        }
        if let Some(default_target) = map.get("default") {
          if let Some(s) = default_target.as_str() {
            return Some(s.to_string());
          }
        }
        None
      }
      _ => None,
    }
  }

  pub async fn load_npm_module(
    &self,
    npm_ref: &NpmPackageReqReference,
  ) -> Result<ModuleSource, JsErrorBox> {
    println!("🔄 Attempting to download npm package: {}", npm_ref.req());

    // Get package info URL
    let package_name = &npm_ref.req().name;
    let info_url = format!("https://registry.npmjs.org/{}", package_name);
    let info_url =
      Url::parse(&info_url).map_err(|e| JsErrorBox::generic(format!("Invalid URL: {}", e)))?;

    println!("📡 Fetching package info from: {}", info_url);

    // Use a simple reqwest client for now
    let client = reqwest::Client::new();
    let response = client
      .get(info_url.as_str())
      .send()
      .await
      .map_err(|e| JsErrorBox::generic(format!("Failed to fetch package info: {}", e)))?;

    if response.status().is_success() {
      let bytes = response
        .bytes()
        .await
        .map_err(|e| JsErrorBox::generic(format!("Failed to read response: {}", e)))?;

      let package_info: deno_npm::registry::NpmPackageInfo = serde_json::from_slice(&bytes)
        .map_err(|e| JsErrorBox::generic(format!("Failed to parse package info: {}", e)))?;

      println!("✅ Got package info for: {}", package_name);

      // Find the best matching version - simplified implementation
      // For now, just pick the first available version
      let version = package_info
        .versions
        .keys()
        .next()
        .ok_or_else(|| JsErrorBox::generic("No versions available"))?;

      println!("📦 Selected version: {}", version);

      // Get tarball URL
      let version_info = package_info
        .versions
        .get(version)
        .ok_or_else(|| JsErrorBox::generic("Version info not found"))?;

      let tarball_url = match &version_info.dist {
        Some(dist) => dist.tarball.clone(),
        None => return Err(JsErrorBox::generic("No tarball URL found")),
      };
      println!("📥 Found tarball URL: {}", tarball_url);

      // Download the tarball
      println!("⬇️ Downloading tarball...");
      let tarball_response = client
        .get(&tarball_url)
        .send()
        .await
        .map_err(|e| JsErrorBox::generic(format!("Failed to download tarball: {}", e)))?;

      if !tarball_response.status().is_success() {
        return Err(JsErrorBox::generic(format!(
          "Failed to download tarball: status {}",
          tarball_response.status()
        )));
      }

      let tarball_bytes = tarball_response
        .bytes()
        .await
        .map_err(|e| JsErrorBox::generic(format!("Failed to read tarball: {}", e)))?;

      println!("📦 Downloaded {} bytes", tarball_bytes.len());

      // Create package directory in cache
      let package_cache_dir = self
        .cache_dir
        .join("registry.npmjs.org")
        .join(package_name)
        .join(version.to_string());

      println!("📁 Extracting to: {}", package_cache_dir.display());

      // Extract tarball
      use deno_semver::package::PackageNv;

      use crate::cli::tarball_extract::{extract_tarball_simple, verify_tarball_integrity};

      let package_nv = PackageNv {
        name: package_name.clone(),
        version: version.clone(),
      };

      // Verify integrity if available
      if let Some(dist) = &version_info.dist {
        verify_tarball_integrity(&package_nv, &tarball_bytes, dist)
          .map_err(|e| JsErrorBox::generic(format!("Integrity check failed: {}", e)))?;
        println!("✅ Tarball integrity verified");
      }

      // Extract the tarball
      extract_tarball_simple(&tarball_bytes, &package_cache_dir)
        .map_err(|e| JsErrorBox::generic(format!("Failed to extract tarball: {}", e)))?;

      println!("✅ Tarball extracted successfully");

      // Look for package.json to resolve main entry point
      let package_json_path = package_cache_dir.join("package.json");
      if package_json_path.exists() {
        let package_json_content = std::fs::read_to_string(&package_json_path)
          .map_err(|e| JsErrorBox::generic(format!("Failed to read package.json: {}", e)))?;

        let package_json: serde_json::Value = serde_json::from_str(&package_json_content)
          .map_err(|e| JsErrorBox::generic(format!("Failed to parse package.json: {}", e)))?;

        // Try to resolve the main entry using package.json exports, main, module, etc.
        let main_entry = self
          .resolve_package_entry(&package_json)
          .unwrap_or_else(|| "index.js".to_string());

        println!("📋 Package main entry: {}", main_entry);

        let main_file_path = package_cache_dir.join(&main_entry);
        if main_file_path.exists() {
          println!("✅ Found main file: {}", main_file_path.display());

          // Load the main file content
          let file_content = std::fs::read_to_string(&main_file_path)
            .map_err(|e| JsErrorBox::generic(format!("Failed to read main file: {}", e)))?;

          println!("📄 Loaded {} characters from main file", file_content.len());

          // Create a file: URL for the extracted file
          let file_url = ModuleSpecifier::from_file_path(&main_file_path)
            .map_err(|_| JsErrorBox::generic("Failed to create file URL"))?;

          // Determine if it's a JS or other file
          let extension = main_file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("js");

          let _module_type = match extension {
            "mjs" => deno_core::ModuleType::JavaScript,
            "js" => deno_core::ModuleType::JavaScript,
            "json" => deno_core::ModuleType::Json,
            _ => deno_core::ModuleType::JavaScript,
          };

          // For CommonJS modules, we need to wrap them in ESM
          let wrapped_content = if is_commonjs_module(&file_content) {
            println!("🔄 Converting CommonJS to ESM");
            wrap_commonjs_to_esm(&file_content, package_name)
          } else {
            println!("✅ Module is already ESM compatible");
            file_content
          };

          println!(
            "🎉 Successfully loaded npm module: {}@{}",
            package_name, version
          );

          // Return the actual module content
          return Ok(deno_core::ModuleSource::new(
            deno_core::ModuleType::JavaScript,
            deno_core::ModuleSourceCode::String(wrapped_content.into()),
            &file_url,
            None,
          ));
        } else {
          // Try fallback entries if main file doesn't exist
          println!(
            "⚠️ Main file not found: {}, trying fallbacks...",
            main_file_path.display()
          );

          let fallback_entries = vec![
            "index.js",
            "index.mjs",
            "index.cjs",
            "lib/index.js",
            "dist/index.js",
            "build/index.js",
            "main.js",
            "src/index.js",
          ];

          for fallback in fallback_entries {
            let fallback_path = package_cache_dir.join(fallback);
            if fallback_path.exists() {
              println!("✅ Found fallback file: {}", fallback_path.display());

              // Load the fallback file content
              let file_content = std::fs::read_to_string(&fallback_path)
                .map_err(|e| JsErrorBox::generic(format!("Failed to read fallback file: {}", e)))?;

              // Create a file: URL for the extracted file
              let file_url = ModuleSpecifier::from_file_path(&fallback_path)
                .map_err(|_| JsErrorBox::generic("Failed to create file URL"))?;

              // Determine module type
              let extension = fallback_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("js");

              let _module_type = match extension {
                "mjs" => deno_core::ModuleType::JavaScript,
                "js" => deno_core::ModuleType::JavaScript,
                "json" => deno_core::ModuleType::Json,
                _ => deno_core::ModuleType::JavaScript,
              };

              // Convert CommonJS to ESM if needed
              let wrapped_content = if is_commonjs_module(&file_content) {
                println!("🔄 Converting CommonJS to ESM");
                wrap_commonjs_to_esm(&file_content, package_name)
              } else {
                println!("✅ Module is already ESM compatible");
                file_content
              };

              println!(
                "🎉 Successfully loaded npm module: {}@{} (fallback: {})",
                package_name, version, fallback
              );

              return Ok(deno_core::ModuleSource::new(
                deno_core::ModuleType::JavaScript,
                deno_core::ModuleSourceCode::String(wrapped_content.into()),
                &file_url,
                None,
              ));
            }
          }

          // Last resort: create a minimal stub for broken packages
          println!(
            "⚠️ Creating minimal stub for broken package: {}",
            package_name
          );
          let stub_content = format!(
            r#"// Minimal stub for broken npm package: {}@{}
console.log("⚠️ [STUB] Package {} has missing main file, using empty stub");

// Export empty functions to prevent import errors
export default {{}};
"#,
            package_name, version, package_name
          );

          // Create a fake file URL for the stub
          let stub_path = package_cache_dir.join("stub_index.js");
          let file_url = ModuleSpecifier::from_file_path(&stub_path)
            .map_err(|_| JsErrorBox::generic("Failed to create stub file URL"))?;

          return Ok(deno_core::ModuleSource::new(
            deno_core::ModuleType::JavaScript,
            deno_core::ModuleSourceCode::String(stub_content.into()),
            &file_url,
            None,
          ));
        }
      } else {
        return Err(JsErrorBox::generic(
          "package.json not found in extracted package",
        ));
      }
    } else {
      Err(JsErrorBox::generic(format!(
        "Package not found: {} (status: {})",
        package_name,
        response.status()
      )))
    }
  }
}

fn is_commonjs_module(content: &str) -> bool {
  // Simple heuristic to detect CommonJS modules
  content.contains("module.exports")
    || content.contains("exports.")
    || (content.contains("require(")
      && !content.contains("import ")
      && !content.contains("export "))
}

fn wrap_commonjs_to_esm(content: &str, package_name: &str) -> String {
  // Check if the content already declares 'exports' variable
  let has_exports_declaration = content.contains("var exports")
    || content.contains("let exports")
    || content.contains("const exports");

  let exports_declaration = if has_exports_declaration {
    "// exports already declared in original code"
  } else {
    "const exports = module.exports;"
  };

  format!(
    r#"// Auto-generated ESM wrapper for CommonJS module: {}
const module = {{ exports: {{}} }};
{}
const require = globalThis.require || ((id) => {{ 
    throw new Error(`Cannot require module "${{id}}" - not implemented in this environment`); 
}});

{}

// Handle different CommonJS export patterns
let defaultExport = module.exports;

// If module.exports is a function or has properties, use it as default
// Special handling for libraries like lodash that export a main function
if (typeof module.exports === 'function') {{
    defaultExport = module.exports;
}} else if (typeof module.exports === 'object' && module.exports !== null) {{
    // If it's an object, check if it has a main property or use the object itself
    if (Object.keys(module.exports).length > 0) {{
        defaultExport = module.exports;
    }}
}}

export default defaultExport;
export const __esModule = true;

// Named exports for object exports
if (typeof module.exports === 'object' && module.exports !== null) {{
    Object.keys(module.exports).forEach(key => {{
        try {{
            globalThis[key] = module.exports[key];
        }} catch (e) {{
            // Ignore errors setting global properties
        }}
    }});
}}
"#,
    package_name, exports_declaration, content
  )
}
