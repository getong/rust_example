use anyhow::Result;
use deno_core_download_example::{NpmConfig, NpmDownloader};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
  // Initialize logging
  tracing_subscriber::fmt()
    .with_env_filter(
      EnvFilter::from_default_env().add_directive("deno_core_download_example=debug".parse()?),
    )
    .with_target(false)
    .init();

  println!("📦 Supabase Package Download Example");
  println!("====================================");

  // Create custom configuration
  let mut config = NpmConfig::default();

  // Use a custom cache directory for this example
  config.cache_dir = std::env::temp_dir().join("supabase-demo-cache");

  println!("🌐 Registry: {}", config.registry_url);
  println!("📁 Cache: {}", config.cache_dir.display());

  // Create downloader
  let downloader = NpmDownloader::new(config)?;

  // Clear cache first (for clean demo)
  if downloader.cache_stats()?.total_packages > 0 {
    println!("🗑️ Clearing cache for clean demo...");
    downloader.cache.clear_all()?;
  }

  // Download Supabase client
  let supabase_spec = "npm:@supabase/supabase-js@2.40.0";
  println!("\n🚀 Downloading: {}", supabase_spec);

  let start_time = std::time::Instant::now();

  match downloader.download_package(supabase_spec).await {
    Ok(cached) => {
      let duration = start_time.elapsed();

      println!("\n✅ Successfully downloaded and cached!");
      println!("   📦 Package: {} v{}", cached.name, cached.version);
      println!("   📁 Location: {}", cached.path.display());
      println!(
        "   📏 Size: {} bytes ({:.2} KB)",
        cached.size,
        cached.size as f64 / 1024.0
      );
      println!("   ⏱️  Time: {:?}", duration);

      if let Some(ref main) = cached.main_entry {
        println!("   🎯 Main entry: {}", main);
      }

      // Try to read and analyze the main entry file
      if let Ok(Some(content)) = downloader.cache.read_main_entry(&cached) {
        println!("\n📄 Main Entry Analysis:");
        println!("   Lines: {}", content.lines().count());
        println!("   Size: {} bytes", content.len());

        // Look for exports
        let exports: Vec<&str> = content
          .lines()
          .filter(|line| line.trim().starts_with("export"))
          .take(5)
          .collect();

        if !exports.is_empty() {
          println!("   📤 Sample exports:");
          for export_line in exports {
            let trimmed = export_line.trim();
            let preview = if trimmed.len() > 60 {
              format!("{}...", &trimmed[.. 57])
            } else {
              trimmed.to_string()
            };
            println!("     {}", preview);
          }
        }

        // Look for imports
        let imports: Vec<&str> = content
          .lines()
          .filter(|line| line.trim().starts_with("import") || line.contains("require("))
          .take(5)
          .collect();

        if !imports.is_empty() {
          println!("   📥 Sample imports:");
          for import_line in imports {
            let trimmed = import_line.trim();
            let preview = if trimmed.len() > 60 {
              format!("{}...", &trimmed[.. 57])
            } else {
              trimmed.to_string()
            };
            println!("     {}", preview);
          }
        }
      }

      // Explore package structure
      println!("\n📂 Package Structure:");
      let package_root = cached.path.join("package");
      if let Ok(entries) = std::fs::read_dir(&package_root) {
        let mut files: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        files.sort();

        for (i, file_path) in files.iter().take(10).enumerate() {
          let name = file_path.file_name().unwrap_or_default().to_string_lossy();

          let file_type = if file_path.is_dir() { "📁" } else { "📄" };
          println!("   {} {}", file_type, name);

          if i == 9 && files.len() > 10 {
            println!("   ... and {} more files", files.len() - 10);
          }
        }
      }

      // Show package.json content
      if cached.package_json_path.exists() {
        println!("\n📋 package.json Summary:");
        if let Ok(package_json_content) = std::fs::read_to_string(&cached.package_json_path) {
          if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&package_json_content)
          {
            if let Some(description) = package_json.get("description").and_then(|d| d.as_str()) {
              println!("   📝 Description: {}", description);
            }

            if let Some(version) = package_json.get("version").and_then(|v| v.as_str()) {
              println!("   🏷️  Version: {}", version);
            }

            if let Some(deps) = package_json.get("dependencies").and_then(|d| d.as_object()) {
              println!("   📦 Dependencies: {} packages", deps.len());
              for (dep_name, _) in deps.iter().take(5) {
                println!("     - {}", dep_name);
              }
              if deps.len() > 5 {
                println!("     ... and {} more", deps.len() - 5);
              }
            }
          }
        }
      }
    }
    Err(e) => {
      println!("❌ Failed to download: {}", e);
      return Err(e);
    }
  }

  // Test caching - download again to show it uses cache
  println!("\n🔄 Testing cache (downloading again)...");
  let cache_start = std::time::Instant::now();

  match downloader.download_package(supabase_spec).await {
    Ok(_cached) => {
      let cache_duration = cache_start.elapsed();
      println!("✅ Retrieved from cache in {:?}", cache_duration);
      println!("   (Original download was much slower than this cache hit!)");
    }
    Err(e) => {
      println!("❌ Cache test failed: {}", e);
    }
  }

  // Final cache stats
  println!("\n📊 Final Cache Stats:");
  let stats = downloader.cache_stats()?;
  println!("   Total packages: {}", stats.total_packages);
  println!(
    "   Total size: {} bytes ({:.2} KB)",
    stats.total_size,
    stats.total_size as f64 / 1024.0
  );

  println!("\n🎯 Use Case: TypeScript/JavaScript Import");
  println!("   This package can now be imported in Deno/TypeScript as:");
  println!("   import {{ createClient }} from \"npm:@supabase/supabase-js@2.40.0\";");
  println!("   ");
  println!("   The runtime would:");
  println!("   1. Parse 'npm:@supabase/supabase-js@2.40.0' → name, version");

  // Get the cached package for display
  if let Ok(packages) = downloader.list_cached() {
    if let Some(supabase_pkg) = packages.iter().find(|p| p.name.contains("supabase")) {
      println!("   2. Check cache → Found: {}", supabase_pkg.path.display());
      println!(
        "   3. Load main entry → {}",
        supabase_pkg.main_entry.as_deref().unwrap_or("index.js")
      );
    }
  }
  println!("   4. Execute JavaScript → Make exports available");

  println!("\n✅ Example completed!");

  Ok(())
}
