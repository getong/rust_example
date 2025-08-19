use std::{
  fs,
  path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::task;

/// Represents a cached package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPackage {
  pub name: String,
  pub version: String,
  pub path: PathBuf,
  pub package_json_path: PathBuf,
  pub main_entry: Option<String>,
  pub cached_at: std::time::SystemTime,
  pub size: u64,
}

/// Cache statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
  pub total_packages: usize,
  pub total_size: u64,
  pub cache_dir: PathBuf,
}

/// NPM package cache manager
pub struct NpmCache {
  pub cache_dir: PathBuf,
  packages_dir: PathBuf,
  metadata_dir: PathBuf,
}

impl NpmCache {
  pub fn new(cache_dir: &Path) -> Result<Self> {
    let packages_dir = cache_dir.join("packages");
    let metadata_dir = cache_dir.join("metadata");

    // Create cache directories
    fs::create_dir_all(&packages_dir)?;
    fs::create_dir_all(&metadata_dir)?;

    Ok(Self {
      cache_dir: cache_dir.to_path_buf(),
      packages_dir,
      metadata_dir,
    })
  }

  /// Check if a package is cached
  pub fn get_package(&self, name: &str, version: &str) -> Result<Option<CachedPackage>> {
    let package_path = self.get_package_path(name, version);
    let metadata_path = self.get_metadata_path(name, version);

    if package_path.exists() && metadata_path.exists() {
      let metadata_str = fs::read_to_string(&metadata_path)?;
      let cached: CachedPackage = serde_json::from_str(&metadata_str)?;
      Ok(Some(cached))
    } else {
      Ok(None)
    }
  }

  /// Store a downloaded package in cache
  pub async fn store_package(
    &self,
    name: &str,
    version: &str,
    tarball_data: &[u8],
  ) -> Result<CachedPackage> {
    let package_path = self.get_package_path(name, version);
    let metadata_path = self.get_metadata_path(name, version);

    // Create package directory
    fs::create_dir_all(&package_path)?;

    // Extract tarball in background thread
    let tarball_data = tarball_data.to_vec();
    let extract_path = package_path.clone();

    task::spawn_blocking(move || Self::extract_tarball(&tarball_data, &extract_path)).await??;

    // Read package.json to get main entry point
    let package_json_path = package_path.join("package").join("package.json");
    let main_entry = if package_json_path.exists() {
      let package_json_str = fs::read_to_string(&package_json_path)?;
      let package_json: serde_json::Value = serde_json::from_str(&package_json_str)?;
      package_json
        .get("main")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
    } else {
      None
    };

    // Calculate directory size
    let size = Self::calculate_directory_size(&package_path)?;

    let cached = CachedPackage {
      name: name.to_string(),
      version: version.to_string(),
      path: package_path,
      package_json_path,
      main_entry,
      cached_at: std::time::SystemTime::now(),
      size,
    };

    // Save metadata
    let metadata_str = serde_json::to_string_pretty(&cached)?;
    fs::write(&metadata_path, metadata_str)?;

    tracing::info!("📦 Cached {} v{} ({} bytes)", name, version, size);

    Ok(cached)
  }

  /// Extract .tar.gz data to directory
  fn extract_tarball(tarball_data: &[u8], extract_path: &Path) -> Result<()> {
    use std::io::Cursor;

    use flate2::read::GzDecoder;
    use tar::Archive;

    let cursor = Cursor::new(tarball_data);
    let gz_decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz_decoder);

    // Extract with safety checks
    for entry in archive.entries()? {
      let mut entry = entry?;
      let path = entry.path()?;

      // Security: Prevent path traversal attacks
      if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
      {
        tracing::warn!("Skipping potentially dangerous path: {:?}", path);
        continue;
      }

      let extract_path = extract_path.join(&path);

      // Create parent directories
      if let Some(parent) = extract_path.parent() {
        fs::create_dir_all(parent)?;
      }

      // Extract file
      entry.unpack(&extract_path)?;
    }

    Ok(())
  }

  /// Calculate total size of directory
  fn calculate_directory_size(dir: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    if dir.is_dir() {
      for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
          total_size += Self::calculate_directory_size(&path)?;
        } else if let Ok(metadata) = entry.metadata() {
          total_size += metadata.len();
        }
      }
    }

    Ok(total_size)
  }

  /// Get package directory path
  fn get_package_path(&self, name: &str, version: &str) -> PathBuf {
    let dir_name = Self::package_dir_name(name, version);
    self.packages_dir.join(dir_name)
  }

  /// Get metadata file path
  fn get_metadata_path(&self, name: &str, version: &str) -> PathBuf {
    let file_name = format!("{}.json", Self::package_dir_name(name, version));
    self.metadata_dir.join(file_name)
  }

  /// Generate safe directory name for package
  fn package_dir_name(name: &str, version: &str) -> String {
    // Replace problematic characters for filesystem
    let safe_name = name.replace('/', "_").replace('@', "");
    format!("{}@{}", safe_name, version)
  }

  /// Get the main entry file path for a cached package
  pub fn get_main_entry_path(&self, cached: &CachedPackage) -> Option<PathBuf> {
    if let Some(ref main_entry) = cached.main_entry {
      let package_root = cached.path.join("package");
      Some(package_root.join(main_entry))
    } else {
      // Default to index.js
      let package_root = cached.path.join("package");
      let default_main = package_root.join("index.js");
      if default_main.exists() {
        Some(default_main)
      } else {
        None
      }
    }
  }

  /// List all cached packages
  pub fn list_packages(&self) -> Result<Vec<CachedPackage>> {
    let mut packages = Vec::new();

    if !self.metadata_dir.exists() {
      return Ok(packages);
    }

    for entry in fs::read_dir(&self.metadata_dir)? {
      let entry = entry?;
      if entry.path().extension() == Some(std::ffi::OsStr::new("json")) {
        match fs::read_to_string(entry.path()) {
          Ok(metadata_str) => match serde_json::from_str::<CachedPackage>(&metadata_str) {
            Ok(cached) => packages.push(cached),
            Err(_) => {} // Skip invalid metadata files
          },
          Err(_) => {} // Skip unreadable files
        }
      }
    }

    // Sort by name and version
    packages.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));

    Ok(packages)
  }
}
