// Copyright 2018-2025 the Deno authors. MIT license.
// Simplified file fetcher based on deno/cli/file_fetcher.rs

use std::{fs, sync::Arc};

use deno_ast::MediaType;
use deno_core::{ModuleSpecifier, error::AnyError};
use deno_error::JsErrorBox;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TextDecodedFile {
  pub media_type: MediaType,
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: ModuleSpecifier,
  /// The source of the file.
  pub source: Arc<str>,
}

#[derive(Debug, Clone)]
pub struct File {
  pub url: ModuleSpecifier,
  pub source: Arc<[u8]>,
  pub media_type: MediaType,
  pub maybe_headers: Option<Vec<(String, String)>>,
}

/// Simple file fetcher that can load local files and mock remote files
#[derive(Clone)]
pub struct SimpleFileFetcher {
  allow_remote: bool,
}

impl SimpleFileFetcher {
  pub fn new(allow_remote: bool) -> Self {
    Self { allow_remote }
  }

  /// Fetch a file from a URL
  pub async fn fetch(&self, specifier: &ModuleSpecifier) -> Result<File, AnyError> {
    match specifier.scheme() {
      "file" => self.fetch_local(specifier).await,
      "http" | "https" if self.allow_remote => self.fetch_remote(specifier).await,
      "http" | "https" => {
        Err(JsErrorBox::generic(format!("Remote modules are not allowed: {}", specifier)).into())
      }
      scheme => Err(JsErrorBox::generic(format!("Unsupported scheme: {}", scheme)).into()),
    }
  }

  /// Fetch a local file
  async fn fetch_local(&self, specifier: &ModuleSpecifier) -> Result<File, AnyError> {
    let path = specifier
      .to_file_path()
      .map_err(|_| JsErrorBox::generic(format!("Invalid file URL: {}", specifier)))?;

    let source = fs::read(&path)
      .map_err(|e| JsErrorBox::generic(format!("Failed to read file {}: {}", path.display(), e)))?;

    let media_type = MediaType::from_path(&path);

    Ok(File {
      url: specifier.clone(),
      source: source.into(),
      media_type,
      maybe_headers: None,
    })
  }

  /// Mock fetch for remote files (returns a placeholder)
  async fn fetch_remote(&self, specifier: &ModuleSpecifier) -> Result<File, AnyError> {
    // In a real implementation, this would use an HTTP client
    // For now, we'll return a mock response
    let mock_source = format!(
      "// Mock remote module: {}\nexport default {{ url: '{}' }};",
      specifier, specifier
    );

    Ok(File {
      url: specifier.clone(),
      source: mock_source.into_bytes().into(),
      media_type: MediaType::JavaScript,
      maybe_headers: Some(vec![
        (
          "content-type".to_string(),
          "application/javascript".to_string(),
        ),
        ("x-mock-response".to_string(), "true".to_string()),
      ]),
    })
  }

  /// Decode a file to text
  pub fn decode(&self, file: File) -> Result<TextDecodedFile, AnyError> {
    let source = String::from_utf8(file.source.to_vec())
      .map_err(|e| JsErrorBox::generic(format!("Failed to decode file: {}", e)))?;

    Ok(TextDecodedFile {
      media_type: file.media_type,
      specifier: file.url,
      source: source.into(),
    })
  }
}

/// Extended file fetcher with caching support
pub struct CachedFileFetcher {
  inner: SimpleFileFetcher,
  cache: std::sync::RwLock<std::collections::HashMap<ModuleSpecifier, File>>,
}

impl CachedFileFetcher {
  pub fn new(allow_remote: bool) -> Self {
    Self {
      inner: SimpleFileFetcher::new(allow_remote),
      cache: std::sync::RwLock::new(std::collections::HashMap::new()),
    }
  }

  pub async fn fetch(&self, specifier: &ModuleSpecifier) -> Result<File, AnyError> {
    // Check cache first
    {
      let cache = self.cache.read().unwrap();
      if let Some(file) = cache.get(specifier) {
        return Ok(file.clone());
      }
    }

    // Fetch the file
    let file = self.inner.fetch(specifier).await?;

    // Cache it
    {
      let mut cache = self.cache.write().unwrap();
      cache.insert(specifier.clone(), file.clone());
    }

    Ok(file)
  }

  pub fn decode(&self, file: File) -> Result<TextDecodedFile, AnyError> {
    self.inner.decode(file)
  }
}
