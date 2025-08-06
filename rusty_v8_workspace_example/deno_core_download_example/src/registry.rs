use std::collections::HashMap;

use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::NpmConfig;

/// NPM package metadata from registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
  pub name: String,
  pub description: Option<String>,
  #[serde(rename = "dist-tags")]
  pub dist_tags: HashMap<String, String>,
  pub versions: HashMap<String, VersionInfo>,
  pub time: Option<HashMap<String, String>>,
}

/// Information about a specific package version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
  pub name: String,
  pub version: String,
  pub description: Option<String>,
  pub main: Option<String>,
  pub scripts: Option<HashMap<String, String>>,
  pub dependencies: Option<HashMap<String, String>>,
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<HashMap<String, String>>,
  pub dist: DistInfo,
}

/// Distribution information (tarball location and integrity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistInfo {
  pub integrity: String,
  pub shasum: String,
  pub tarball: String,
  #[serde(rename = "fileCount")]
  pub file_count: Option<u32>,
  #[serde(rename = "unpackedSize")]
  pub unpacked_size: Option<u64>,
}

/// NPM registry client
pub struct NpmRegistry {
  client: Client,
  base_url: String,
  auth_header: Option<String>,
}

impl NpmRegistry {
  pub fn new(config: &NpmConfig) -> Result<Self> {
    let client = Client::builder().user_agent(&config.user_agent).build()?;

    let auth_header = config
      .auth_token
      .as_ref()
      .map(|token| format!("Bearer {}", token));

    Ok(Self {
      client,
      base_url: config.registry_url.clone(),
      auth_header,
    })
  }

  /// Fetch package metadata from NPM registry
  pub async fn get_package_metadata(&self, package_name: &str) -> Result<PackageMetadata> {
    let url = format!("{}/{}", self.base_url, package_name.replace('/', "%2F"));

    tracing::debug!("🌐 Fetching metadata from: {}", url);

    let mut request = self.client.get(&url).header("Accept", "application/json");

    if let Some(ref auth) = self.auth_header {
      request = request.header("Authorization", auth);
    }

    let response = request.send().await?;

    match response.status() {
      StatusCode::OK => {
        let metadata: PackageMetadata = response.json().await?;
        Ok(metadata)
      }
      StatusCode::NOT_FOUND => Err(anyhow!("Package '{}' not found in registry", package_name)),
      StatusCode::UNAUTHORIZED => Err(anyhow!(
        "Authentication required for package '{}'",
        package_name
      )),
      status => {
        let error_text = response.text().await.unwrap_or_default();
        Err(anyhow!("Registry error {}: {}", status, error_text))
      }
    }
  }

  /// Download package tarball
  pub async fn download_tarball(&self, tarball_url: &str) -> Result<Vec<u8>> {
    tracing::debug!("📦 Downloading tarball: {}", tarball_url);

    let mut request = self.client.get(tarball_url);

    if let Some(ref auth) = self.auth_header {
      request = request.header("Authorization", auth);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
      return Err(anyhow!("Failed to download tarball: {}", response.status()));
    }

    let content_length = response.content_length();
    if let Some(length) = content_length {
      tracing::debug!("📊 Tarball size: {} bytes", length);
    }

    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
  }

  /// Search for packages (bonus feature)
  pub async fn search_packages(&self, query: &str, size: u32) -> Result<SearchResult> {
    let search_url = format!("{}/-/v1/search", self.base_url);
    let url = Url::parse_with_params(&search_url, &[("text", query), ("size", &size.to_string())])?;

    tracing::debug!("🔍 Searching packages: {}", url);

    let response = self.client.get(url).send().await?;

    if !response.status().is_success() {
      return Err(anyhow!("Search failed: {}", response.status()));
    }

    let result: SearchResult = response.json().await?;
    Ok(result)
  }

  /// Get registry statistics (bonus feature)
  pub async fn get_registry_info(&self) -> Result<RegistryInfo> {
    let info_url = format!("{}/", self.base_url);

    let response = self.client.get(&info_url).send().await?;

    if !response.status().is_success() {
      return Err(anyhow!(
        "Failed to get registry info: {}",
        response.status()
      ));
    }

    let info: RegistryInfo = response.json().await?;
    Ok(info)
  }
}

/// Search result from NPM registry
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
  pub objects: Vec<SearchObject>,
  pub total: u32,
  pub time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchObject {
  pub package: SearchPackage,
  pub score: SearchScore,
  #[serde(rename = "searchScore")]
  pub search_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchPackage {
  pub name: String,
  pub scope: Option<String>,
  pub version: String,
  pub description: Option<String>,
  pub keywords: Option<Vec<String>>,
  pub date: String,
  pub links: Option<SearchLinks>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchScore {
  pub final_score: f64,
  pub detail: SearchScoreDetail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchScoreDetail {
  pub quality: f64,
  pub popularity: f64,
  pub maintenance: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchLinks {
  pub npm: Option<String>,
  pub homepage: Option<String>,
  pub repository: Option<String>,
  pub bugs: Option<String>,
}

/// Registry information
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryInfo {
  pub db_name: String,
  pub doc_count: u64,
  pub doc_del_count: u64,
  pub update_seq: u64,
  pub purge_seq: u64,
  pub compact_running: bool,
  pub disk_size: u64,
  pub data_size: u64,
  pub instance_start_time: String,
}

#[cfg(test)]
mod tests {
  use tokio_test;

  use super::*;

  #[tokio_test::tokio::test]
  async fn test_get_package_metadata() {
    let config = NpmConfig::default();
    let registry = NpmRegistry::new(&config).unwrap();

    // Test with a well-known package
    let result = registry.get_package_metadata("lodash").await;
    assert!(result.is_ok());

    let metadata = result.unwrap();
    assert_eq!(metadata.name, "lodash");
    assert!(metadata.versions.len() > 0);
    assert!(metadata.dist_tags.contains_key("latest"));
  }

  #[tokio_test::tokio::test]
  async fn test_scoped_package_metadata() {
    let config = NpmConfig::default();
    let registry = NpmRegistry::new(&config).unwrap();

    // Test with a scoped package
    let result = registry.get_package_metadata("@types/node").await;
    assert!(result.is_ok());

    let metadata = result.unwrap();
    assert_eq!(metadata.name, "@types/node");
  }

  #[tokio_test::tokio::test]
  async fn test_nonexistent_package() {
    let config = NpmConfig::default();
    let registry = NpmRegistry::new(&config).unwrap();

    let result = registry
      .get_package_metadata("this-package-should-not-exist-123456")
      .await;
    assert!(result.is_err());
  }

  #[tokio_test::tokio::test]
  async fn test_search_packages() {
    let config = NpmConfig::default();
    let registry = NpmRegistry::new(&config).unwrap();

    let result = registry.search_packages("lodash", 5).await;
    assert!(result.is_ok());

    let search_result = result.unwrap();
    assert!(search_result.objects.len() > 0);
    assert!(
      search_result
        .objects
        .iter()
        .any(|obj| obj.package.name == "lodash")
    );
  }
}
