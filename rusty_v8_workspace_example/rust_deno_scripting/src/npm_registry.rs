use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::npm_downloader::NpmConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
  pub name: String,
  pub versions: HashMap<String, VersionInfo>,
  #[serde(rename = "dist-tags")]
  pub dist_tags: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionInfo {
  pub name: String,
  pub version: String,
  pub dependencies: Option<HashMap<String, String>>,
  #[serde(rename = "devDependencies")]
  pub dev_dependencies: Option<HashMap<String, String>>,
  #[serde(rename = "peerDependencies")]
  pub peer_dependencies: Option<HashMap<String, String>>,
  pub dist: DistInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DistInfo {
  pub tarball: String,
  pub integrity: String,
  pub shasum: String,
}

pub struct NpmRegistry {
  client: reqwest::Client,
  config: NpmConfig,
}

impl NpmRegistry {
  pub fn new(config: &NpmConfig) -> Result<Self> {
    let mut client_builder = reqwest::Client::builder()
      .user_agent(&config.user_agent)
      .timeout(std::time::Duration::from_secs(30));

    if let Some(token) = &config.auth_token {
      let mut headers = reqwest::header::HeaderMap::new();
      headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))?,
      );
      client_builder = client_builder.default_headers(headers);
    }

    let client = client_builder.build()?;

    Ok(Self {
      client,
      config: config.clone(),
    })
  }

  pub async fn get_package_metadata(&self, package_name: &str) -> Result<PackageMetadata> {
    let url = format!("{}/{}", self.config.registry_url, package_name);

    tracing::info!("Fetching metadata from: {}", url);

    let response = self.client.get(&url).send().await?.error_for_status()?;

    let metadata: PackageMetadata = response.json().await?;

    Ok(metadata)
  }

  pub async fn download_tarball(&self, tarball_url: &str) -> Result<Vec<u8>> {
    tracing::info!("Downloading tarball from: {}", tarball_url);

    let response = self
      .client
      .get(tarball_url)
      .send()
      .await?
      .error_for_status()?;

    let bytes = response.bytes().await?;

    Ok(bytes.to_vec())
  }
}
