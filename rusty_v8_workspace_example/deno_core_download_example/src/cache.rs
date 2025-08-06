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
            Err(e) => tracing::warn!("Failed to parse metadata file {:?}: {}", entry.path(), e),
          },
          Err(e) => tracing::warn!("Failed to read metadata file {:?}: {}", entry.path(), e),
        }
      }
    }

    // Sort by name and version
    packages.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));

    Ok(packages)
  }

  /// Remove a package from cache
  pub fn remove_package(&self, name: &str) -> Result<()> {
    let mut removed_count = 0;

    // Find all versions of this package
    for entry in fs::read_dir(&self.packages_dir)? {
      let entry = entry?;
      let dir_name = entry.file_name();
      let dir_name_str = dir_name.to_string_lossy();

      if Self::package_dir_matches(name, &dir_name_str) {
        let package_path = entry.path();
        let metadata_file = format!("{}.json", dir_name_str);
        let metadata_path = self.metadata_dir.join(metadata_file);

        // Remove package directory
        if package_path.is_dir() {
          fs::remove_dir_all(&package_path)?;
          removed_count += 1;
        }

        // Remove metadata file
        if metadata_path.exists() {
          fs::remove_file(&metadata_path)?;
        }
      }
    }

    tracing::info!(
      "🗑️ Removed {} cached versions of package '{}'",
      removed_count,
      name
    );
    Ok(())
  }

  /// Get cache statistics
  pub fn get_stats(&self) -> Result<CacheStats> {
    let packages = self.list_packages()?;
    let total_size = packages.iter().map(|p| p.size).sum();

    Ok(CacheStats {
      total_packages: packages.len(),
      total_size,
      cache_dir: self.cache_dir.clone(),
    })
  }

  /// Clear entire cache
  pub fn clear_all(&self) -> Result<()> {
    if self.packages_dir.exists() {
      fs::remove_dir_all(&self.packages_dir)?;
      fs::create_dir_all(&self.packages_dir)?;
    }

    if self.metadata_dir.exists() {
      fs::remove_dir_all(&self.metadata_dir)?;
      fs::create_dir_all(&self.metadata_dir)?;
    }

    tracing::info!("🗑️ Cleared entire package cache");
    Ok(())
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

  /// Check if directory name matches package name
  fn package_dir_matches(package_name: &str, dir_name: &str) -> bool {
    let safe_name = package_name.replace('/', "_").replace('@', "");
    dir_name.starts_with(&format!("{}@", safe_name))
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

  /// Read the main entry file content
  pub fn read_main_entry(&self, cached: &CachedPackage) -> Result<Option<String>> {
    if let Some(main_path) = self.get_main_entry_path(cached) {
      if main_path.exists() {
        let content = fs::read_to_string(&main_path)?;
        Ok(Some(content))
      } else {
        Ok(None)
      }
    } else {
      Ok(None)
    }
  }
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_package_dir_name() {
    assert_eq!(
      NpmCache::package_dir_name("lodash", "4.17.21"),
      "lodash@4.17.21"
    );
    assert_eq!(
      NpmCache::package_dir_name("@types/node", "18.0.0"),
      "types_node@18.0.0"
    );
    assert_eq!(
      NpmCache::package_dir_name("@supabase/supabase-js", "2.40.0"),
      "supabase_supabase-js@2.40.0"
    );
  }

  #[test]
  fn test_package_dir_matches() {
    assert!(NpmCache::package_dir_matches("lodash", "lodash@4.17.21"));
    assert!(NpmCache::package_dir_matches(
      "@types/node",
      "types_node@18.0.0"
    ));
    assert!(NpmCache::package_dir_matches(
      "@supabase/supabase-js",
      "supabase_supabase-js@2.40.0"
    ));

    assert!(!NpmCache::package_dir_matches("lodash", "express@4.18.0"));
  }

  #[tokio::test]
  async fn test_cache_creation() {
    let temp_dir = TempDir::new().unwrap();
    let cache = NpmCache::new(temp_dir.path()).unwrap();

    assert!(cache.packages_dir.exists());
    assert!(cache.metadata_dir.exists());
  }
}
