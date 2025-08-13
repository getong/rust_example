use std::collections::HashMap;

use anyhow::{Result, anyhow};
use reqwest::{
  Client,
  header::{HeaderMap, HeaderValue, USER_AGENT},
};
use serde::{Deserialize, Serialize};

/// NPM package metadata from registry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageMetadata {
  pub name: String,
  pub description: Option<String>,
  #[serde(rename = "dist-tags")]
  pub dist_tags: HashMap<String, String>,
  pub versions: HashMap<String, VersionInfo>,
}

/// Version-specific package information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionInfo {
  pub name: String,
  pub version: String,
  pub description: Option<String>,
  pub main: Option<String>,
  pub dist: DistInfo,
  pub dependencies: Option<HashMap<String, String>>,
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<HashMap<String, String>>,
}

/// Distribution information (tarball URL, etc.)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DistInfo {
  pub tarball: String,
  pub shasum: String,
  pub integrity: String,
  #[serde(rename = "unpackedSize")]
  pub unpacked_size: Option<u64>,
}

/// NPM registry client
pub struct NpmRegistry {
  client: Client,
  registry_url: String,
}

impl NpmRegistry {
  pub fn new(config: &crate::npm_downloader::NpmConfig) -> Result<Self> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_str(&config.user_agent)?);
    headers.insert("Accept", HeaderValue::from_static("application/json"));

    if let Some(ref token) = config.auth_token {
      let auth_value = format!("Bearer {}", token);
      headers.insert("Authorization", HeaderValue::from_str(&auth_value)?);
    }

    let client = Client::builder()
      .default_headers(headers)
      .timeout(std::time::Duration::from_secs(30))
      .build()?;

    Ok(Self {
      client,
      registry_url: config.registry_url.clone(),
    })
  }

  /// Fetch package metadata from npm registry
  pub async fn get_package_metadata(&self, package_name: &str) -> Result<PackageMetadata> {
    // URL encode the package name to handle scoped packages like @types/node
    let encoded_name = urlencoding::encode(package_name);
    let url = format!("{}/{}", self.registry_url, encoded_name);

    println!("🌐 Fetching metadata for {} from {}", package_name, url);

    let response = self.client.get(&url).send().await?;

    if !response.status().is_success() {
      return Err(anyhow!(
        "Failed to fetch package metadata: HTTP {}",
        response.status()
      ));
    }

    let metadata: PackageMetadata = response.json().await?;
    Ok(metadata)
  }

  /// Download package tarball
  pub async fn download_tarball(&self, tarball_url: &str) -> Result<Vec<u8>> {
    tracing::info!("📥 Downloading tarball from {}", tarball_url);

    let response = self.client.get(tarball_url).send().await?;

    if !response.status().is_success() {
      return Err(anyhow!(
        "Failed to download tarball: HTTP {}",
        response.status()
      ));
    }

    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
  }
}
