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
  pub config: NpmConfig,
  registry: NpmRegistry,
  pub cache: NpmCache,
}

impl NpmDownloader {
  pub fn new(config: NpmConfig) -> Result<Self> {
    let registry = NpmRegistry::new(&config)?;
    let cache = NpmCache::new(&config.cache_dir)?;

    Ok(Self {
      config,
      registry,
      cache,
    })
  }

  /// Download and cache a package from npm: specifier
  pub async fn download_package(&self, specifier: &str) -> Result<CachedPackage> {
    tracing::info!("🚀 Starting download for: {}", specifier);

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
