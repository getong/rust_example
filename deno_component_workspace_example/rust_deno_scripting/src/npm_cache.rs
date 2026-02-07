use std::{
  fs,
  path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::task;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
  pub total_packages: usize,
  pub total_size: u64,
  pub cache_dir: PathBuf,
}

pub struct NpmCache {
  pub cache_dir: PathBuf,
  packages_dir: PathBuf,
  metadata_dir: PathBuf,
}

impl NpmCache {
  pub fn new(cache_dir: &Path) -> Result<Self> {
    let packages_dir = cache_dir.join("packages");
    let metadata_dir = cache_dir.join("metadata");

    fs::create_dir_all(&packages_dir)?;
    fs::create_dir_all(&metadata_dir)?;

    Ok(Self {
      cache_dir: cache_dir.to_path_buf(),
      packages_dir,
      metadata_dir,
    })
  }

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

  pub async fn store_package(
    &self,
    name: &str,
    version: &str,
    tarball_data: &[u8],
  ) -> Result<CachedPackage> {
    let package_path = self.get_package_path(name, version);
    let metadata_path = self.get_metadata_path(name, version);

    fs::create_dir_all(&package_path)?;

    let tarball_data = tarball_data.to_vec();
    let extract_path = package_path.clone();

    task::spawn_blocking(move || Self::extract_tarball(&tarball_data, &extract_path)).await??;

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

    let metadata_str = serde_json::to_string_pretty(&cached)?;
    fs::write(&metadata_path, metadata_str)?;

    tracing::info!("📦 Cached {} v{} ({} bytes)", name, version, size);

    Ok(cached)
  }

  fn extract_tarball(tarball_data: &[u8], extract_path: &Path) -> Result<()> {
    use std::io::Cursor;

    use flate2::read::GzDecoder;
    use tar::Archive;

    let cursor = Cursor::new(tarball_data);
    let gz_decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz_decoder);

    for entry in archive.entries()? {
      let mut entry = entry?;
      let path = entry.path()?;

      if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
      {
        tracing::warn!("Skipping potentially dangerous path: {:?}", path);
        continue;
      }

      let extract_path = extract_path.join(&path);

      if let Some(parent) = extract_path.parent() {
        fs::create_dir_all(parent)?;
      }

      entry.unpack(&extract_path)?;
    }

    Ok(())
  }

  fn calculate_directory_size(path: &Path) -> Result<u64> {
    let mut size = 0;
    for entry in walkdir::WalkDir::new(path) {
      let entry = entry?;
      if entry.file_type().is_file() {
        size += entry.metadata()?.len();
      }
    }
    Ok(size)
  }

  fn get_package_path(&self, name: &str, version: &str) -> PathBuf {
    let safe_name = name.replace('/', "-");
    self.packages_dir.join(format!("{}-{}", safe_name, version))
  }

  fn get_metadata_path(&self, name: &str, version: &str) -> PathBuf {
    let safe_name = name.replace('/', "-");
    self
      .metadata_dir
      .join(format!("{}-{}.json", safe_name, version))
  }

  pub fn get_main_entry_path(&self, cached_package: &CachedPackage) -> Option<PathBuf> {
    if let Some(main_entry) = &cached_package.main_entry {
      Some(cached_package.path.join("package").join(main_entry))
    } else {
      None
    }
  }

  pub fn list_packages(&self) -> Result<Vec<CachedPackage>> {
    let mut packages = Vec::new();

    if let Ok(entries) = fs::read_dir(&self.metadata_dir) {
      for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
          if let Ok(metadata_str) = fs::read_to_string(&path) {
            if let Ok(cached) = serde_json::from_str::<CachedPackage>(&metadata_str) {
              packages.push(cached);
            }
          }
        }
      }
    }

    Ok(packages)
  }

  pub fn stats(&self) -> Result<CacheStats> {
    let packages = self.list_packages()?;
    let total_packages = packages.len();
    let total_size = packages.iter().map(|p| p.size).sum();

    Ok(CacheStats {
      total_packages,
      total_size,
      cache_dir: self.cache_dir.clone(),
    })
  }
}
