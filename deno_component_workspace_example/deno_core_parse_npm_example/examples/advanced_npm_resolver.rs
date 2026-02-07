// Advanced example showing how to implement a more complete npm resolver
// This is closer to what edge-runtime actually does, but simplified

use std::{collections::HashMap, rc::Rc};

use anyhow::{Result, anyhow};
use deno_core::{
  FastString, JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoader, ModuleSource,
  ModuleSpecifier, ModuleType, ResolutionKind, RuntimeOptions, error::JsErrorBox,
};
use deno_semver::npm::NpmPackageReqReference;

/// A simplified NPM module loader that demonstrates the concepts
/// used in edge-runtime for loading npm: packages
pub struct SimpleNpmModuleLoader {
  /// Simulated npm package cache
  package_cache: HashMap<String, String>,
}

impl SimpleNpmModuleLoader {
  pub fn new() -> Self {
    let mut cache = HashMap::new();

    // Simulate some cached npm packages
    cache.insert(
      "lodash@4.17.21".to_string(),
      r#"
// Simulated lodash module
export function isArray(value) {
    return Array.isArray(value);
}

export function isEmpty(value) {
    return value == null || (typeof value === 'object' && Object.keys(value).length === 0);
}

export default { isArray, isEmpty };
"#
      .to_string(),
    );

    cache.insert(
      "@supabase/supabase-js@2.40.0".to_string(),
      r#"
// Simulated @supabase/supabase-js module
export function createClient(url, key, options = {}) {
    console.log('Creating Supabase client for:', url);
    return {
        from: (table) => ({
            select: () => Promise.resolve({ data: [], error: null }),
            insert: () => Promise.resolve({ data: [], error: null }),
        }),
        auth: {
            signIn: () => Promise.resolve({ user: null, error: null }),
        }
    };
}

export default { createClient };
"#
      .to_string(),
    );

    Self {
      package_cache: cache,
    }
  }

  /// Resolve npm: specifier to a cached package
  fn resolve_npm_specifier(&self, specifier: &str) -> Result<String> {
    // Parse the npm: specifier
    let npm_ref = NpmPackageReqReference::from_str(specifier)
      .map_err(|e| anyhow!("Failed to parse npm specifier {}: {}", specifier, e))?;

    let req = npm_ref.into_inner();
    let package_key = format!("{}@{}", req.req.name, req.req.version_req);

    // Look up in our simulated cache
    self
      .package_cache
      .get(&package_key)
      .cloned()
      .ok_or_else(|| anyhow!("Package not found in cache: {}", package_key))
  }
}

impl ModuleLoader for SimpleNpmModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    println!("🔍 Resolving: {} (from: {})", specifier, referrer);

    // Check if this is an npm: specifier
    if specifier.starts_with("npm:") {
      // For npm: specifiers, we return them as-is for loading
      return ModuleSpecifier::parse(specifier).map_err(Into::into);
    }

    // Handle relative/absolute imports normally
    if specifier.starts_with("./") || specifier.starts_with("../") {
      let base = ModuleSpecifier::parse(referrer)?;
      return base.join(specifier).map_err(Into::into);
    }

    // Absolute URLs
    ModuleSpecifier::parse(specifier).map_err(Into::into)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> deno_core::ModuleLoadResponse {
    let specifier_str = module_specifier.as_str();

    println!("📦 Loading: {}", specifier_str);

    // Handle npm: specifiers
    if specifier_str.starts_with("npm:") {
      match self.resolve_npm_specifier(specifier_str) {
        Ok(source_code) => {
          let module_source = ModuleSource::new(
            ModuleType::JavaScript,
            FastString::from(source_code).into(),
            module_specifier,
            None,
          );
          deno_core::ModuleLoadResponse::Sync(Ok(module_source))
        }
        Err(e) => deno_core::ModuleLoadResponse::Sync(Err(JsErrorBox::from_err(e))),
      }
    } else {
      // For this example, we don't handle file: specifiers
      let error = anyhow!("Unsupported module specifier: {}", specifier_str);
      deno_core::ModuleLoadResponse::Sync(Err(JsErrorBox::from_err(error)))
    }
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  println!("🚀 Advanced NPM Module Loader Example");
  println!("=====================================");

  // Create our custom module loader
  let module_loader = Rc::new(SimpleNpmModuleLoader::new());

  // Test resolution
  println!("\n🔍 Testing Module Resolution:");

  let test_specifiers = vec![
    "npm:lodash@4.17.21",
    "npm:@supabase/supabase-js@2.40.0",
    "./local-module.js",
    "https://deno.land/std/path/mod.ts",
  ];

  for spec in test_specifiers {
    match module_loader.resolve(spec, "file:///test.js", ResolutionKind::Import) {
      Ok(resolved) => println!("✅ {} → {}", spec, resolved),
      Err(e) => println!("❌ {} → Error: {}", spec, e),
    }
  }

  // Test with a runtime that uses our custom module loader
  println!("\n🚀 Testing with JsRuntime:");

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(module_loader.clone()),
    ..Default::default()
  });

  // JavaScript code that would import npm packages
  let js_code = r#"
        console.log("🦕 Advanced NPM Module Loading Simulation");
        
        // This demonstrates what would happen with actual npm: imports
        const npmSpecifiers = [
            "npm:lodash@4.17.21",
            "npm:@supabase/supabase-js@2.40.0"
        ];
        
        console.log("Would attempt to import from these npm: specifiers:");
        for (const spec of npmSpecifiers) {
            console.log(`  - ${spec}`);
            
            // Parse the specifier (what happens in our Rust code)
            const withoutPrefix = spec.replace("npm:", "");
            let packageName, version;
            
            if (withoutPrefix.startsWith("@")) {
                // Scoped package
                const parts = withoutPrefix.split("@");
                packageName = "@" + parts[1];
                version = parts[2] || "latest";
            } else {
                // Regular package
                const parts = withoutPrefix.split("@");
                packageName = parts[0];
                version = parts[1] || "latest";
            }
            
            console.log(`    → Package: ${packageName}`);
            console.log(`    → Version: ${version}`);
        }
        
        console.log("\n✅ NPM loading simulation complete");
        "Advanced example finished successfully";
    "#;

  println!("Executing JavaScript with custom module loader...");
  let result = runtime.execute_script("advanced_example.js", js_code)?;

  let result_str = {
    deno_core::scope!(scope, &mut runtime);
    let result_local = deno_core::v8::Local::new(scope, result);
    result_local.to_rust_string_lossy(scope)
  };
  println!("Script result: {}", result_str);

  runtime.run_event_loop(Default::default()).await?;

  println!("✅ Advanced example completed!");

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_npm_resolution() {
    let loader = SimpleNpmModuleLoader::new();

    let result = loader.resolve(
      "npm:lodash@4.17.21",
      "file:///test.js",
      ResolutionKind::Import,
    );

    assert!(result.is_ok());
    let specifier = result.unwrap();
    assert_eq!(specifier.scheme(), "npm");
  }

  #[test]
  fn test_npm_specifier_cache_lookup() {
    let loader = SimpleNpmModuleLoader::new();

    let result = loader.resolve_npm_specifier("npm:lodash@4.17.21");
    assert!(result.is_ok());

    let source_code = result.unwrap();
    assert!(source_code.contains("export function isArray"));
  }

  #[test]
  fn test_scoped_package() {
    let loader = SimpleNpmModuleLoader::new();

    let result = loader.resolve_npm_specifier("npm:@supabase/supabase-js@2.40.0");
    assert!(result.is_ok());

    let source_code = result.unwrap();
    assert!(source_code.contains("export function createClient"));
  }
}
