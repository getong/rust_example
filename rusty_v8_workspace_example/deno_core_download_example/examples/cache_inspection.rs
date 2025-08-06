use anyhow::Result;
use deno_core_download_example::{CachedPackage, NpmConfig, NpmDownloader};
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

  println!("🔍 NPM Cache Inspection Tool");
  println!("============================");

  // Create downloader
  let config = NpmConfig::default();
  let downloader = NpmDownloader::new(config)?;

  println!(
    "📁 Cache directory: {}",
    downloader.cache.cache_dir.display()
  );

  // Get current cache stats
  let stats = downloader.cache_stats()?;
  println!("\n📊 Cache Statistics:");
  println!("   Total packages: {}", stats.total_packages);
  println!(
    "   Total size: {} bytes ({:.2} MB)",
    stats.total_size,
    stats.total_size as f64 / (1024.0 * 1024.0)
  );

  if stats.total_packages == 0 {
    println!("\n💡 Cache is empty. Let's download some packages for inspection...");

    let sample_packages = vec!["npm:lodash@4.17.21", "npm:chalk@5.3.0", "npm:is-even@1.0.0"];

    for package_spec in sample_packages {
      println!("\n📦 Downloading {} for inspection...", package_spec);
      match downloader.download_package(package_spec).await {
        Ok(cached) => {
          println!("✅ Cached {} v{}", cached.name, cached.version);
        }
        Err(e) => {
          println!("❌ Failed to download {}: {}", package_spec, e);
        }
      }
    }
  }

  // List all cached packages
  println!("\n📋 Cached Packages:");
  let packages = downloader.list_cached()?;

  if packages.is_empty() {
    println!("   (No packages in cache)");
    return Ok(());
  }

  // Clone packages for analysis later
  let packages_for_analysis = packages.clone();

  // Group packages by name for better display
  let mut packages_by_name = std::collections::HashMap::new();
  for package in packages {
    packages_by_name
      .entry(package.name.clone())
      .or_insert_with(Vec::new)
      .push(package);
  }

  for (name, versions) in &packages_by_name {
    println!("\n📦 {} ({} versions)", name, versions.len());

    for package in versions {
      println!("   └─ v{} ({} bytes)", package.version, package.size);
      println!("      📁 Path: {}", package.path.display());

      if let Some(ref main) = package.main_entry {
        println!("      🎯 Main: {}", main);
      }

      // Show cached time
      if let Ok(duration) = package.cached_at.elapsed() {
        println!("      ⏰ Cached: {:.0} seconds ago", duration.as_secs());
      }

      // Inspect package structure
      inspect_package_structure(&package);
    }
  }

  // Analyze cache usage patterns
  println!("\n📈 Cache Analysis:");
  let total_packages = packages_for_analysis.len();
  let total_size: u64 = packages_for_analysis.iter().map(|p| p.size).sum();
  let avg_size = if total_packages > 0 {
    total_size / total_packages as u64
  } else {
    0
  };

  println!(
    "   Average package size: {} bytes ({:.2} KB)",
    avg_size,
    avg_size as f64 / 1024.0
  );

  // Find largest packages
  let mut packages_by_size = packages_for_analysis;
  packages_by_size.sort_by_key(|p| std::cmp::Reverse(p.size));

  if !packages_by_size.is_empty() {
    println!("\n🏆 Largest packages:");
    for (i, package) in packages_by_size.iter().take(5).enumerate() {
      println!(
        "   {}. {} v{} - {} bytes ({:.2} KB)",
        i + 1,
        package.name,
        package.version,
        package.size,
        package.size as f64 / 1024.0
      );
    }
  }

  // Show package types distribution
  println!("\n📊 Package Types:");
  let mut scoped_count = 0;
  let mut regular_count = 0;

  for package in &packages_by_size {
    if package.name.starts_with('@') {
      scoped_count += 1;
    } else {
      regular_count += 1;
    }
  }

  println!("   Scoped packages (@org/name): {}", scoped_count);
  println!("   Regular packages: {}", regular_count);

  // Suggest cleanup if needed
  let cache_size_mb = total_size as f64 / (1024.0 * 1024.0);
  if cache_size_mb > 100.0 {
    println!("\n💡 Cache cleanup suggestion:");
    println!(
      "   Your cache is {:.2} MB. Consider clearing old packages:",
      cache_size_mb
    );
    println!("   - Use downloader.clear_cache(\"package-name\") for specific packages");
    println!("   - Use downloader.cache.clear_all() to clear everything");
  }

  // Show cache directories
  println!("\n📂 Cache Directory Structure:");
  println!("   📁 {}", downloader.cache.cache_dir.display());
  println!("   ├─ 📁 packages/     (extracted package files)");
  println!("   └─ 📁 metadata/     (package metadata cache)");

  println!("\n✅ Cache inspection completed!");

  Ok(())
}

fn inspect_package_structure(package: &CachedPackage) {
  let package_root = package.path.join("package");

  if let Ok(entries) = std::fs::read_dir(&package_root) {
    let mut file_types = std::collections::HashMap::new();
    let mut total_files = 0;

    for entry in entries.flatten() {
      total_files += 1;

      if let Some(extension) = entry.path().extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        *file_types.entry(ext).or_insert(0) += 1;
      } else if entry.path().is_dir() {
        *file_types.entry("dir".to_string()).or_insert(0) += 1;
      } else {
        *file_types.entry("no-ext".to_string()).or_insert(0) += 1;
      }
    }

    if total_files > 0 {
      print!("      📂 Files: {} (", total_files);
      let mut type_strs = Vec::new();
      for (ext, count) in file_types.iter().take(3) {
        type_strs.push(format!("{} {}", count, ext));
      }
      print!("{}", type_strs.join(", "));
      if file_types.len() > 3 {
        print!(", +{} more types", file_types.len() - 3);
      }
      println!(")");
    }
  }

  // Check if it has TypeScript definitions
  let ts_defs = package_root.join("index.d.ts");
  if ts_defs.exists() {
    println!("      📝 TypeScript definitions available");
  }

  // Check if it's ESM or CommonJS
  if let Ok(package_json_content) = std::fs::read_to_string(&package.package_json_path) {
    if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&package_json_content) {
      if let Some(module_type) = package_json.get("type").and_then(|t| t.as_str()) {
        if module_type == "module" {
          println!("      📦 ESM module");
        } else {
          println!("      📦 CommonJS module");
        }
      } else {
        println!("      📦 CommonJS module (default)");
      }
    }
  }
}
