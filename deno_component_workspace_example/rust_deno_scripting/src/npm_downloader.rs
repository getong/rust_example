use std::{collections::HashMap, path::PathBuf};

use anyhow::{Result, anyhow};

use crate::{
  npm_cache::{CachedPackage, NpmCache},
  npm_registry::{NpmRegistry, PackageMetadata},
  npm_specifier::NpmSpecifier,
};

#[derive(Debug, Clone)]
pub struct NpmConfig {
  pub registry_url: String,
  pub cache_dir: PathBuf,
  pub auth_token: Option<String>,
  pub user_agent: String,
}

impl Default for NpmConfig {
  fn default() -> Self {
    let cache_dir = dirs::cache_dir()
      .unwrap_or_else(|| std::env::temp_dir())
      .join("rust_deno_npm_cache");

    Self {
      registry_url: "https://registry.npmjs.org".to_string(),
      cache_dir,
      auth_token: std::env::var("NPM_TOKEN").ok(),
      user_agent: "rust_deno_scripting/0.1.0".to_string(),
    }
  }
}

pub struct NpmDownloader {
  registry: NpmRegistry,
  pub cache: NpmCache,
}

impl NpmDownloader {
  pub fn new(config: NpmConfig) -> Result<Self> {
    tracing::debug!(
      "NpmDownloader config: registry_url={} cache_dir={} user_agent={}",
      config.registry_url,
      config.cache_dir.display(),
      config.user_agent
    );
    let registry = NpmRegistry::new(&config)?;
    let cache = NpmCache::new(&config.cache_dir)?;

    Ok(Self { registry, cache })
  }

  pub async fn download_package(&self, specifier: &str) -> Result<CachedPackage> {
    tracing::info!("🚀 Starting download for: {}", specifier);

    if let Ok(stats) = self.cache.stats() {
      tracing::debug!(
        "NPM cache stats: total_packages={} total_size={}B cache_dir={}",
        stats.total_packages,
        stats.total_size,
        stats.cache_dir.display()
      );
    }

    let npm_spec = NpmSpecifier::parse(specifier)?;
    tracing::info!(
      "📦 Parsed package: {} version: {:?}",
      npm_spec.name,
      npm_spec.version
    );

    let metadata = self.registry.get_package_metadata(&npm_spec.name).await?;
    tracing::info!(
      "📥 Fetched metadata for {} ({} versions)",
      npm_spec.name,
      metadata.versions.len()
    );

    let resolved_version = self.resolve_version(&npm_spec, &metadata)?;
    tracing::info!("🔍 Resolved version: {}", resolved_version);

    if let Some(cached) = self.cache.get_package(&npm_spec.name, &resolved_version)? {
      println!("✅ Found in cache: {}", cached.path.display());
      return Ok(cached);
    }

    println!(
      "📦 Package {} v{} not in cache, downloading...",
      npm_spec.name, resolved_version
    );

    let version_info = metadata.versions.get(&resolved_version).ok_or_else(|| {
      anyhow!(
        "Version {} not found for {}",
        resolved_version,
        npm_spec.name
      )
    })?;

    let tarball_data = self
      .registry
      .download_tarball(&version_info.dist.tarball)
      .await?;
    tracing::info!("⬇️  Downloaded tarball: {} bytes", tarball_data.len());

    self.verify_integrity(&tarball_data, &version_info.dist.integrity)?;
    tracing::info!("🔐 Verified package integrity");

    let cached = self
      .cache
      .store_package(&npm_spec.name, &resolved_version, &tarball_data)
      .await?;
    tracing::info!("💾 Cached package at: {}", cached.path.display());

    Ok(cached)
  }

  pub async fn download_package_with_dependencies(&self, specifier: &str) -> Result<CachedPackage> {
    use std::collections::HashSet;

    tracing::info!("🌐 Starting recursive download for: {}", specifier);
    let mut downloaded_packages = HashSet::new();

    self
      .download_package_recursive_with_depth(specifier, &mut downloaded_packages, 0, 3)
      .await
  }

  fn download_package_recursive_with_depth<'a>(
    &'a self,
    specifier: &'a str,
    downloaded: &'a mut std::collections::HashSet<String>,
    current_depth: u32,
    max_depth: u32,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CachedPackage>> + 'a>> {
    Box::pin(async move {
      if current_depth >= max_depth {
        println!("🔢 Reached max depth ({}) for: {}", max_depth, specifier);
        return self.download_package(specifier).await;
      }

      if downloaded.contains(specifier) {
        tracing::info!("⚠️  Skipping already processed: {}", specifier);
        let npm_spec = NpmSpecifier::parse(specifier)?;
        let metadata = self.registry.get_package_metadata(&npm_spec.name).await?;
        let resolved_version = self.resolve_version(&npm_spec, &metadata)?;
        return self
          .cache
          .get_package(&npm_spec.name, &resolved_version)?
          .ok_or_else(|| anyhow!("Package should be cached: {}", specifier));
      }

      let cached_package = self.download_package(specifier).await?;
      downloaded.insert(specifier.to_string());

      let package_json_path = cached_package.path.join("package").join("package.json");
      println!(
        "📋 [Depth {}] Looking for package.json at: {}",
        current_depth,
        package_json_path.display()
      );
      if let Ok(package_json_content) = std::fs::read_to_string(&package_json_path) {
        println!(
          "✅ [Depth {}] Successfully read package.json for {}",
          current_depth, cached_package.name
        );
        if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&package_json_content) {
          let dependencies = self.extract_all_dependencies(&package_json);

          println!(
            "📦 [Depth {}] Found {} dependencies for {}: {:?}",
            current_depth,
            dependencies.len(),
            cached_package.name,
            dependencies.keys().collect::<Vec<_>>()
          );

          for (dep_name, version_spec) in dependencies {
            let dep_specifier = if version_spec.starts_with("npm:") {
              version_spec
            } else {
              format!("npm:{}@{}", dep_name, version_spec)
            };

            println!(
              "🔄 [Depth {}] Downloading dependency: {}",
              current_depth + 1,
              dep_specifier
            );
            if let Err(e) = self
              .download_package_recursive_with_depth(
                &dep_specifier,
                downloaded,
                current_depth + 1,
                max_depth,
              )
              .await
            {
              tracing::warn!(
                "⚠️  Failed to download dependency {} of {}: {}",
                dep_specifier,
                cached_package.name,
                e
              );
            }
          }
        } else {
          tracing::warn!(
            "⚠️  Failed to parse package.json for {}",
            cached_package.name
          );
        }
      } else {
        println!(
          "⚠️  [Depth {}] No package.json found for {}",
          current_depth, cached_package.name
        );
      }

      Ok(cached_package)
    })
  }

  fn resolve_version(&self, npm_spec: &NpmSpecifier, metadata: &PackageMetadata) -> Result<String> {
    if npm_spec.version == "latest" {
      metadata
        .dist_tags
        .get("latest")
        .cloned()
        .ok_or_else(|| anyhow!("No 'latest' tag found for {}", npm_spec.name))
    } else {
      // For now, use exact version matching
      // TODO: Implement proper semver resolution
      if metadata.versions.contains_key(&npm_spec.version) {
        Ok(npm_spec.version.clone())
      } else {
        Err(anyhow!(
          "Version {} not found for {}",
          npm_spec.version,
          npm_spec.name
        ))
      }
    }
  }

  fn verify_integrity(&self, data: &[u8], expected_integrity: &str) -> Result<()> {
    use base64::{Engine as _, engine::general_purpose};
    use sha2::{Digest, Sha512};

    if expected_integrity.starts_with("sha512-") {
      let expected_hash = expected_integrity
        .strip_prefix("sha512-")
        .ok_or_else(|| anyhow!("Invalid integrity format"))?;

      let mut hasher = Sha512::new();
      hasher.update(data);
      let result = hasher.finalize();

      let actual_hash = general_purpose::STANDARD.encode(&result);

      if actual_hash == expected_hash {
        Ok(())
      } else {
        Err(anyhow!("Integrity check failed"))
      }
    } else {
      // For now, skip other hash types
      tracing::warn!(
        "Skipping integrity check for format: {}",
        expected_integrity
      );
      Ok(())
    }
  }

  fn extract_all_dependencies(&self, package_json: &serde_json::Value) -> HashMap<String, String> {
    let mut all_deps = HashMap::new();

    if let Some(deps) = package_json.get("dependencies").and_then(|d| d.as_object()) {
      for (name, version) in deps {
        if let Some(version_str) = version.as_str() {
          all_deps.insert(name.clone(), version_str.to_string());
        }
      }
    }

    // Optionally include peer dependencies
    if let Some(deps) = package_json
      .get("peerDependencies")
      .and_then(|d| d.as_object())
    {
      for (name, version) in deps {
        if let Some(version_str) = version.as_str() {
          if !all_deps.contains_key(name) {
            all_deps.insert(name.clone(), version_str.to_string());
          }
        }
      }
    }

    all_deps
  }
}
