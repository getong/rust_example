use std::path::PathBuf;

use anyhow::{Result, anyhow};

use crate::{
  npm_cache::{CachedPackage, NpmCache},
  npm_registry::{NpmRegistry, PackageMetadata},
  npm_specifier::NpmSpecifier,
};

/// Configuration for NPM package downloading
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
      .join("deno_npm_cache");

    Self {
      registry_url: "https://registry.npmjs.org".to_string(),
      cache_dir,
      auth_token: std::env::var("NPM_TOKEN").ok(),
      user_agent: "deno_runtime_npm_downloader/0.1.0".to_string(),
    }
  }
}

/// Main NPM package downloader
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

  /// Download and cache a package from npm: specifier
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

    // Parse the npm: specifier
    let npm_spec = NpmSpecifier::parse(specifier)?;
    tracing::info!(
      "📦 Parsed package: {} version: {:?}",
      npm_spec.name,
      npm_spec.version
    );

    // Fetch package metadata from registry
    let metadata = self.registry.get_package_metadata(&npm_spec.name).await?;
    tracing::info!(
      "📥 Fetched metadata for {} ({} versions)",
      npm_spec.name,
      metadata.versions.len()
    );

    // Resolve version
    let resolved_version = self.resolve_version(&npm_spec, &metadata)?;
    tracing::info!("🔍 Resolved version: {}", resolved_version);

    // Check if already cached
    if let Some(cached) = self.cache.get_package(&npm_spec.name, &resolved_version)? {
      tracing::info!("✅ Found in cache: {}", cached.path.display());
      return Ok(cached);
    }

    let version_info = metadata.versions.get(&resolved_version).ok_or_else(|| {
      anyhow!(
        "Version {} not found for {}",
        resolved_version,
        npm_spec.name
      )
    })?;

    // Download tarball
    let tarball_data = self
      .registry
      .download_tarball(&version_info.dist.tarball)
      .await?;
    tracing::info!("⬇️  Downloaded tarball: {} bytes", tarball_data.len());

    // Verify integrity
    self.verify_integrity(&tarball_data, &version_info.dist.integrity)?;
    tracing::info!("🔐 Verified package integrity");

    // Extract and cache
    let cached = self
      .cache
      .store_package(&npm_spec.name, &resolved_version, &tarball_data)
      .await?;
    tracing::info!("💾 Cached package at: {}", cached.path.display());

    Ok(cached)
  }

  /// Download package with all its dependencies recursively
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
      // Check depth limit
      if current_depth >= max_depth {
        println!("🔢 Reached max depth ({}) for: {}", max_depth, specifier);
        // Still download the main package but don't recurse
        return self.download_package(specifier).await;
      }

      // Avoid circular dependencies
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

      // Download the main package first
      let cached_package = self.download_package(specifier).await?;
      downloaded.insert(specifier.to_string());

      // Parse package.json to get dependencies
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

          // Download each dependency recursively
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
              println!("⚠️  Failed to download dependency {}: {}", dep_specifier, e);
              // Continue with other dependencies even if one fails
            } else {
              println!("✅ Successfully downloaded dependency: {}", dep_specifier);
            }
          }
        }
      }

      Ok(cached_package)
    })
  }

  /// Extract runtime dependencies from package.json (only dependencies, not dev/peer)
  fn extract_all_dependencies(
    &self,
    package_json: &serde_json::Value,
  ) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;

    let mut all_deps = HashMap::new();

    // Extract only regular dependencies (not dev dependencies to avoid downloading too many
    // packages)
    if let Some(deps) = package_json.get("dependencies").and_then(|d| d.as_object()) {
      for (name, version) in deps {
        if let Some(version_str) = version.as_str() {
          all_deps.insert(name.clone(), version_str.to_string());
        }
      }
    }

    // Only extract TypeScript type definitions from dev dependencies (for TypeScript support)
    if let Some(dev_deps) = package_json
      .get("devDependencies")
      .and_then(|d| d.as_object())
    {
      for (name, version) in dev_deps {
        if name.starts_with("@types/") {
          if let Some(version_str) = version.as_str() {
            all_deps.insert(name.clone(), version_str.to_string());
          }
        }
      }
    }

    all_deps
  }

  /// Resolve version constraint to specific version
  fn resolve_version(&self, spec: &NpmSpecifier, metadata: &PackageMetadata) -> Result<String> {
    match &spec.version {
      Some(version_req) => {
        // Parse all available versions
        let mut versions: Vec<semver::Version> = Vec::new();
        for version_str in metadata.versions.keys() {
          if let Ok(version) = semver::Version::parse(version_str) {
            versions.push(version);
          }
        }
        versions.sort();
        versions.reverse(); // Highest first

        // Find matching version
        let req = semver::VersionReq::parse(version_req)?;
        for version in &versions {
          if req.matches(version) {
            return Ok(version.to_string());
          }
        }

        Err(anyhow!(
          "No matching version found for {} {}",
          spec.name,
          version_req
        ))
      }
      None => {
        // Get latest version
        metadata
          .dist_tags
          .get("latest")
          .cloned()
          .ok_or_else(|| anyhow!("No latest version found for {}", spec.name))
      }
    }
  }

  /// Verify package integrity using SHA-512
  fn verify_integrity(&self, data: &[u8], integrity: &str) -> Result<()> {
    if integrity.starts_with("sha512-") {
      use base64::{Engine, engine::general_purpose::STANDARD};
      use sha2::{Digest, Sha512};

      let expected_hash = &integrity[7 ..]; // Remove "sha512-" prefix
      let expected_bytes = STANDARD.decode(expected_hash)?;

      let mut hasher = Sha512::new();
      hasher.update(data);
      let actual_bytes = hasher.finalize();

      if actual_bytes.as_slice() == expected_bytes {
        Ok(())
      } else {
        Err(anyhow!("Package integrity check failed"))
      }
    } else {
      tracing::warn!("Unsupported integrity format: {}", integrity);
      Ok(()) // Allow for now
    }
  }
}
