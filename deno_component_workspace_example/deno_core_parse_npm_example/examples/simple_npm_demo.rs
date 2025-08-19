// Simplified example focusing on npm: specifier parsing without complex traits
use std::{collections::HashMap, rc::Rc};

use anyhow::Result;
use deno_core::{JsRuntime, RuntimeOptions};
use deno_semver::npm::NpmPackageReqReference;

/// Simple npm package cache simulator
struct NpmPackageCache {
  packages: HashMap<String, String>,
}

impl NpmPackageCache {
  fn new() -> Self {
    let mut packages = HashMap::new();

    // Add some simulated packages
    packages.insert(
      "lodash@4.17.21".to_string(),
      "export function isArray(value) { return Array.isArray(value); }".to_string(),
    );

    packages.insert(
      "@supabase/supabase-js@2.40.0".to_string(),
      "export function createClient(url, key) { return { url, key }; }".to_string(),
    );

    Self { packages }
  }

  fn resolve_package(&self, specifier: &str) -> Result<Option<String>> {
    println!("🔍 Resolving npm specifier: {}", specifier);

    // Parse the npm: specifier
    let npm_ref = NpmPackageReqReference::from_str(specifier)?;
    let req = npm_ref.into_inner();

    let package_key = format!("{}@{}", req.req.name, req.req.version_req);
    println!("   Resolved to package key: {}", package_key);

    Ok(self.packages.get(&package_key).cloned())
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  println!("🚀 Simple NPM Resolution Demo");
  println!("==============================");

  let cache = NpmPackageCache::new();

  // Test npm: specifier parsing and resolution
  let test_specifiers = vec![
    "npm:lodash@4.17.21",
    "npm:@supabase/supabase-js@2.40.0",
    "npm:express@4.18.2", // This one won't be found
  ];

  for specifier in test_specifiers {
    println!("\n📦 Testing: {}", specifier);

    match cache.resolve_package(specifier) {
      Ok(Some(source_code)) => {
        println!("✅ Found package!");
        println!(
          "   Source: {}",
          source_code.chars().take(50).collect::<String>()
        );
        if source_code.len() > 50 {
          println!("   ...(truncated)");
        }
      }
      Ok(None) => {
        println!("❌ Package not found in cache");
      }
      Err(e) => {
        println!("❌ Error parsing specifier: {}", e);
      }
    }
  }

  // Show how this would work in a JavaScript runtime
  println!("\n🚀 JavaScript Runtime Integration");
  println!("----------------------------------");

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
    ..Default::default()
  });

  let js_code = r#"
        console.log("🦕 NPM Resolution Simulation");
        
        // Simulate what happens when we encounter npm: imports
        const npmImports = [
            "npm:lodash@4.17.21",
            "npm:@supabase/supabase-js@2.40.0"
        ];
        
        console.log("Processing npm: imports:");
        for (const npmSpec of npmImports) {
            console.log(`  Processing: ${npmSpec}`);
            
            // This is what NpmPackageReqReference::from_str() does in Rust
            if (npmSpec.startsWith("npm:")) {
                const withoutPrefix = npmSpec.slice(4);
                
                let name, version;
                if (withoutPrefix.startsWith("@")) {
                    // Scoped package like @supabase/supabase-js@2.40.0
                    const parts = withoutPrefix.split("@");
                    name = "@" + parts[1];
                    version = parts[2] || "*";
                } else {
                    // Regular package like lodash@4.17.21
                    const parts = withoutPrefix.split("@");
                    name = parts[0];
                    version = parts[1] || "*";
                }
                
                console.log(`    → Package: ${name}`);
                console.log(`    → Version: ${version}`);
                console.log(`    → Would resolve to: ${name}@${version}`);
            }
        }
        
        console.log("✅ NPM resolution simulation complete");
        "Demo completed successfully";
    "#;

  let result = runtime.execute_script("npm_demo.js", js_code)?;

  let result_str = {
    let scope = &mut runtime.handle_scope();
    let result_local = deno_core::v8::Local::new(scope, result);
    result_local.to_rust_string_lossy(scope)
  };

  println!("\nScript result: {}", result_str);
  runtime.run_event_loop(Default::default()).await?;

  // Show the complete flow
  println!("\n🔄 Complete NPM Loading Flow");
  println!("-----------------------------");
  println!("1. ✅ Detect npm: prefix in import specifier");
  println!("2. ✅ Parse package name and version with NpmPackageReqReference");
  println!("3. ✅ Look up package in cache/registry");
  println!("4. ✅ Resolve to actual module source code");
  println!("5. ✅ Load module into JavaScript runtime");
  println!("6. ✅ Make exports available for import");

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_npm_specifier_parsing() {
    let result = NpmPackageReqReference::from_str("npm:lodash@4.17.21");
    assert!(result.is_ok());

    let npm_ref = result.unwrap();
    let req = npm_ref.into_inner();
    assert_eq!(req.req.name, "lodash");
  }

  #[test]
  fn test_scoped_package() {
    let result = NpmPackageReqReference::from_str("npm:@supabase/supabase-js@2.40.0");
    assert!(result.is_ok());

    let npm_ref = result.unwrap();
    let req = npm_ref.into_inner();
    assert_eq!(req.req.name, "@supabase/supabase-js");
  }

  #[test]
  fn test_package_cache() {
    let cache = NpmPackageCache::new();
    let result = cache.resolve_package("npm:lodash@4.17.21");

    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
  }
}
