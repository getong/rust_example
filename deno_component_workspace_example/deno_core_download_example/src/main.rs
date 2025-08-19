use anyhow::Result;
use deno_core_download_example::{NpmConfig, NpmDownloader};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  // Initialize logging
  tracing_subscriber::fmt()
    .with_env_filter(
      EnvFilter::from_default_env().add_directive("deno_core_download_example=info".parse()?),
    )
    .with_target(false)
    .init();

  println!("🚀 NPM Package Downloader Demo");
  println!("===============================");

  // Create configuration
  let config = NpmConfig::default();
  println!("📁 Cache directory: {}", config.cache_dir.display());
  println!("🌐 Registry: {}", config.registry_url);

  // Create downloader
  let downloader = NpmDownloader::new(config)?;

  // Show cache stats before
  println!("\n📊 Cache Stats (Before):");
  let stats = downloader.cache_stats()?;
  println!("   Packages: {}", stats.total_packages);
  println!("   Size: {} bytes", stats.total_size);

  // Demo packages to download
  let packages = vec![
    "npm:lodash@4.17.21",
    "npm:@supabase/supabase-js@2.40.0",
    "npm:is-even@1.0.0",
    "npm:chalk@5.3.0",
  ];

  println!("\n🔄 Downloading packages...");

  for package_spec in packages {
    println!("\n📦 Processing: {}", package_spec);

    match downloader.download_package(package_spec).await {
      Ok(cached) => {
        println!(
          "✅ Downloaded packagespec: {}, {} v{}, cached package json path: {:?}",
          package_spec, cached.name, cached.version, cached.package_json_path
        );
        println!("   Path: {}", cached.path.display());
        println!("   Size: {} bytes", cached.size);

        if let Some(ref main) = cached.main_entry {
          println!("   Main: {}", main);
        }

        // Try to read main entry
        if let Ok(Some(content)) = downloader.cache.read_main_entry(&cached) {
          let preview = content.lines().take(3).collect::<Vec<_>>().join("\\n");
          println!(
            "   Preview: {}...",
            preview.chars().take(100).collect::<String>()
          );
        }
      }
      Err(e) => {
        println!("❌ Failed to download {}: {}", package_spec, e);
      }
    }
  }

  // Show cache stats after
  println!("\n📊 Cache Stats (After):");
  let stats = downloader.cache_stats()?;
  println!("   Packages: {}", stats.total_packages);
  println!("   Size: {} bytes", stats.total_size);

  // List cached packages
  println!("\n📋 Cached Packages:");
  let cached_packages = downloader.list_cached()?;
  for package in cached_packages {
    println!(
      "   {} v{} ({} bytes)",
      package.name, package.version, package.size
    );
  }

  // Demonstrate specifier parsing
  println!("\n🔍 Specifier Parsing Examples:");
  let examples = vec![
    "npm:lodash",
    "npm:lodash@4.17.21",
    "npm:@types/node@18.0.0",
    "npm:@supabase/supabase-js@^2.0.0",
    "npm:lodash/fp",
    "npm:@types/node/fs",
  ];

  for example in examples {
    match deno_core_download_example::NpmSpecifier::parse(example) {
      Ok(spec) => {
        println!(
          "   {} → name: '{}', version: {:?}, subpath: {:?}",
          example, spec.name, spec.version, spec.sub_path
        );
      }
      Err(e) => {
        println!("   {} → Error: {}", example, e);
      }
    }
  }

  // Demonstrate how this would integrate with a module system
  println!("\n🔗 Module Resolution Example:");
  println!(
    "   When encountering: import {{ createClient }} from \"npm:@supabase/supabase-js@2.40.0\";"
  );
  println!("   1. Parse specifier → @supabase/supabase-js v2.40.0");
  println!("   2. Check cache → Found/Not found");
  println!("   3. Download if needed → Registry → Tarball → Extract → Cache");
  println!("   4. Resolve main entry → package.json.main or index.js");
  println!("   5. Load module content → Transform if needed → Execute");

  println!("\n✅ Demo completed successfully!");

  Ok(())
}
