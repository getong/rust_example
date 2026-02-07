use std::rc::Rc;

use anyhow::Result;
use deno_core::{JsRuntime, RuntimeOptions};
use deno_semver::npm::NpmPackageReqReference;

#[tokio::main]
async fn main() -> Result<()> {
  println!("🦕 Deno NPM Package Loading Example");
  println!("====================================");

  // Example 1: Parse npm: specifier
  demonstrate_npm_specifier_parsing()?;

  // Example 2: Create a basic Deno runtime
  demonstrate_basic_runtime().await?;

  Ok(())
}

/// Demonstrates how to parse npm: specifiers
fn demonstrate_npm_specifier_parsing() -> Result<()> {
  println!("\n📦 1. NPM Specifier Parsing");
  println!("---------------------------");

  // Parse various npm: specifiers
  let specifiers = vec![
    "npm:@supabase/supabase-js@2.40.0",
    "npm:lodash@4.17.21",
    "npm:express",
    "npm:is-even@1.0.0",
  ];

  for specifier_str in specifiers {
    match NpmPackageReqReference::from_str(specifier_str) {
      Ok(npm_ref) => {
        let req = npm_ref.into_inner();
        println!("✅ Parsed: {}", specifier_str);
        println!("   Package: {}", req.req.name);
        println!("   Version: {:?}", req.req.version_req);
        println!("   Sub path: {:?}", req.sub_path);
      }
      Err(e) => {
        println!("❌ Failed to parse {}: {}", specifier_str, e);
      }
    }
    println!();
  }

  Ok(())
}

/// Demonstrates creating a basic Deno runtime
async fn demonstrate_basic_runtime() -> Result<()> {
  println!("🚀 2. Basic Deno Runtime");
  println!("-------------------------");

  // Create a basic JsRuntime
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
    ..Default::default()
  });

  // JavaScript code that demonstrates npm: concepts
  let js_code = r#"
        console.log("🦕 Understanding npm: loading process...");

        // This is what we want to achieve conceptually:
        // import { createClient } from "npm:@supabase/supabase-js@2.40.0";

        const npmSpecifier = "npm:@supabase/supabase-js@2.40.0";
        console.log("NPM Specifier:", npmSpecifier);

        // Simulate the parsing process that happens in Rust
        function parseNpmSpecifier(spec) {
            if (!spec.startsWith("npm:")) {
                throw new Error("Not an npm specifier");
            }

            const withoutPrefix = spec.slice(4); // Remove "npm:"
            const parts = withoutPrefix.split("@");

            if (withoutPrefix.startsWith("@")) {
                // Scoped package like @supabase/supabase-js@2.40.0
                const name = "@" + parts[1];
                const version = parts[2] || "latest";
                return { name, version };
            } else {
                // Regular package like lodash@4.17.21
                const name = parts[0];
                const version = parts[1] || "latest";
                return { name, version };
            }
        }

        try {
            const parsed = parseNpmSpecifier(npmSpecifier);
            console.log("Parsed package name:", parsed.name);
            console.log("Parsed version:", parsed.version);

            console.log("\n🔄 Resolution Process:");
            console.log("1. Detect npm: prefix ✓");
            console.log("2. Parse package name and version ✓");
            console.log("3. Would resolve to npm cache location");
            console.log("4. Would load and transform module");
            console.log("5. Would make available for import");

            globalThis.result = "NPM specifier parsing demonstration complete";
        } catch (error) {
            console.error("Error:", error.message);
        }
    "#;

  // Execute the JavaScript
  println!("Executing JavaScript demonstration...");
  let result = runtime.execute_script("npm_example.js", js_code)?;

  // Handle the result
  let result_str = {
    deno_core::scope!(scope, &mut runtime);
    let result_local = deno_core::v8::Local::new(scope, result);
    result_local.to_rust_string_lossy(scope)
  };
  println!("Script result: {}", result_str);

  // Run the event loop to completion
  runtime.run_event_loop(Default::default()).await?;

  println!("✅ Runtime execution completed successfully!");

  Ok(())
}

/// Helper function to create a basic module specifier
// fn create_npm_specifier(package: &str) -> Result<ModuleSpecifier> {
//     let npm_url = format!("npm:{}", package);
//     ModuleSpecifier::parse(&npm_url).map_err(Into::into)
// }

/// Demonstrates various npm: specifier formats
// fn demonstrate_specifier_formats() -> Result<()> {
//     println!("\n🔍 3. NPM Specifier Formats");
//     println!("----------------------------");

//     let test_cases = vec![
//         ("npm:lodash", "Simple package, latest version"),
//         ("npm:lodash@4.17.21", "Package with specific version"),
//         ("npm:@types/node", "Scoped package, latest version"),
//         ("npm:@supabase/supabase-js@2.40.0", "Scoped package with version"),
//         ("npm:express@^4.18.0", "Package with semver range"),
//     ];

//     for (specifier, description) in test_cases {
//         match create_npm_specifier(&specifier[4..]) {
//             Ok(module_spec) => {
//                 println!("✅ {}: {}", specifier, description);
//                 println!("   Scheme: {}", module_spec.scheme());
//                 println!("   Path: {}", module_spec.path());
//             }
//             Err(e) => {
//                 println!("❌ {}: Error - {}", specifier, e);
//             }
//         }
//         println!();
//     }

//     Ok(())
// }

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
  fn test_scoped_package_parsing() {
    let result = NpmPackageReqReference::from_str("npm:@supabase/supabase-js@2.40.0");
    assert!(result.is_ok());

    let npm_ref = result.unwrap();
    let req = npm_ref.into_inner();
    assert_eq!(req.req.name, "@supabase/supabase-js");
  }

  #[test]
  fn test_create_npm_specifier() {
    let specifier = create_npm_specifier("express@4.18.2").unwrap();
    assert_eq!(specifier.scheme(), "npm");
    assert_eq!(specifier.path(), "express@4.18.2");
  }

  #[test]
  fn test_invalid_specifier() {
    let result = NpmPackageReqReference::from_str("invalid:package");
    assert!(result.is_err());
  }
}
