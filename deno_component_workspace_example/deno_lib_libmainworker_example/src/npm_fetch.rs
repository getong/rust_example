// NPM package fetching using deno_fetch
// Based on Deno CLI's npm fetching implementation

use std::{collections::HashMap, sync::Arc};

use deno_core::{anyhow::Context, error::AnyError};
use deno_error::JsErrorBox;
use deno_runtime::{
  deno_fetch::{self, CreateHttpClientOptions, ReqBody, create_http_client},
  deno_tls::RootCertStoreProvider,
};

/// HTTP client for fetching npm packages
pub struct NpmHttpClient {
  client: deno_fetch::Client,
  root_cert_store_provider: Arc<dyn RootCertStoreProvider>,
}

impl NpmHttpClient {
  pub fn new(root_cert_store_provider: Arc<dyn RootCertStoreProvider>) -> Result<Self, AnyError> {
    let options = CreateHttpClientOptions {
      root_cert_store: Some(root_cert_store_provider.get_or_try_init()?.clone()),
      unsafely_ignore_certificate_errors: None,
      ca_certs: vec![],
      proxy: None,
      client_cert_chain_and_key: None,
      ..Default::default()
    };

    let client = create_http_client("deno/npm-example", options)?;

    Ok(Self {
      client,
      root_cert_store_provider,
    })
  }

  /// Fetch npm package metadata from the registry
  pub async fn fetch_package_info(&self, package_name: &str) -> Result<String, AnyError> {
    // NPM registry URL for package info
    let url = format!("https://registry.npmjs.org/{}", package_name);

    println!("Fetching npm package info from: {}", url);

    // Create HTTP request
    let mut request = http::Request::new(ReqBody::empty());
    *request.method_mut() = http::Method::GET;
    *request.uri_mut() = url.parse()?;

    // Add headers
    request.headers_mut().insert(
      http::header::ACCEPT,
      http::HeaderValue::from_static("application/json"),
    );

    // Send request
    let response = self
      .client
      .clone()
      .send(request)
      .await
      .map_err(|e| JsErrorBox::from_err(e))?;

    // Check status
    if !response.status().is_success() {
      return Err(
        JsErrorBox::generic(format!(
          "Failed to fetch npm package: {} - {}",
          package_name,
          response.status()
        ))
        .into(),
      );
    }

    // Read response body
    use http_body_util::BodyExt;
    let body = response
      .into_body()
      .collect()
      .await
      .map_err(|e| JsErrorBox::from_err(e))?;
    let bytes = body.to_bytes();

    let json_str =
      String::from_utf8(bytes.to_vec()).context("Failed to decode npm package response")?;

    Ok(json_str)
  }

  /// Fetch a specific version of an npm package tarball
  pub async fn fetch_package_tarball(
    &self,
    package_name: &str,
    version: &str,
  ) -> Result<Vec<u8>, AnyError> {
    // First fetch package info to get tarball URL
    let package_info = self.fetch_package_info(package_name).await?;
    let info: serde_json::Value = serde_json::from_str(&package_info)?;

    // Get tarball URL for the specific version
    let tarball_url = info["versions"][version]["dist"]["tarball"]
      .as_str()
      .ok_or_else(|| {
        JsErrorBox::generic(format!(
          "Tarball URL not found for {}@{}",
          package_name, version
        ))
      })?;

    println!("Fetching npm tarball from: {}", tarball_url);

    // Create HTTP request for tarball
    let mut request = http::Request::new(ReqBody::empty());
    *request.method_mut() = http::Method::GET;
    *request.uri_mut() = tarball_url.parse()?;

    // Send request
    let response = self
      .client
      .clone()
      .send(request)
      .await
      .map_err(|e| JsErrorBox::from_err(e))?;

    // Check status
    if !response.status().is_success() {
      return Err(
        JsErrorBox::generic(format!(
          "Failed to fetch npm tarball: {} - {}",
          tarball_url,
          response.status()
        ))
        .into(),
      );
    }

    // Read response body
    use http_body_util::BodyExt;
    let body = response
      .into_body()
      .collect()
      .await
      .map_err(|e| JsErrorBox::from_err(e))?;
    let bytes = body.to_bytes();

    Ok(bytes.to_vec())
  }
}

/// Simple npm package resolver that fetches from registry
pub struct NpmPackageResolver {
  http_client: NpmHttpClient,
  cache: HashMap<String, serde_json::Value>,
}

impl NpmPackageResolver {
  pub fn new(root_cert_store_provider: Arc<dyn RootCertStoreProvider>) -> Result<Self, AnyError> {
    Ok(Self {
      http_client: NpmHttpClient::new(root_cert_store_provider)?,
      cache: HashMap::new(),
    })
  }

  /// Resolve an npm package to its metadata
  pub async fn resolve_package(
    &mut self,
    package_name: &str,
  ) -> Result<serde_json::Value, AnyError> {
    // Check cache first
    if let Some(cached) = self.cache.get(package_name) {
      return Ok(cached.clone());
    }

    // Fetch from registry
    let package_json = self.http_client.fetch_package_info(package_name).await?;
    let package_data: serde_json::Value = serde_json::from_str(&package_json)?;

    // Cache the result
    self
      .cache
      .insert(package_name.to_string(), package_data.clone());

    Ok(package_data)
  }

  /// Get the latest version of a package
  pub async fn get_latest_version(&mut self, package_name: &str) -> Result<String, AnyError> {
    let package_data = self.resolve_package(package_name).await?;

    let latest_version = package_data["dist-tags"]["latest"]
      .as_str()
      .ok_or_else(|| {
        JsErrorBox::generic(format!(
          "Latest version not found for package: {}",
          package_name
        ))
      })?;

    Ok(latest_version.to_string())
  }

  /// Get package metadata for a specific version
  pub async fn get_package_version(
    &mut self,
    package_name: &str,
    version: &str,
  ) -> Result<serde_json::Value, AnyError> {
    let package_data = self.resolve_package(package_name).await?;

    let version_data = package_data["versions"][version].clone();
    if version_data.is_null() {
      return Err(
        JsErrorBox::generic(format!(
          "Version {} not found for package: {}",
          version, package_name
        ))
        .into(),
      );
    }

    Ok(version_data)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_npm_http_client() {
    // This would test the HTTP client if we had a test server
    // For now, it's a placeholder
  }
}
