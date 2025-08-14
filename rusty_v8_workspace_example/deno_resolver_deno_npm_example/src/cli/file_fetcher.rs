// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_ast::MediaType;
use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_cache_dir::file_fetcher::BlobData;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::File;
use deno_cache_dir::file_fetcher::SendError;
use deno_cache_dir::file_fetcher::SendResponse;
use deno_core::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_resolver::file_fetcher::PermissionedFileFetcherOptions;
use deno_runtime::deno_web::BlobStore;
use http::HeaderMap;
use http::StatusCode;

use crate::cli::http_util::HttpClientProvider;
use crate::cli::http_util::get_response_body_with_progress;
use crate::cli::CliSys;
use crate::cli::util::progress_bar::ProgressBar;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TextDecodedFile {
  pub media_type: MediaType,
  /// The _final_ specifier for the file.  The requested specifier and the final
  /// specifier maybe different for remote files that have been redirected.
  pub specifier: ModuleSpecifier,
  /// The source of the file.
  pub source: Arc<str>,
}

impl TextDecodedFile {
  /// Decodes the source bytes into a string handling any encoding rules
  /// for local vs remote files and dealing with the charset.
  pub fn decode(file: File) -> Result<Self, AnyError> {
    let (media_type, maybe_charset) =
      deno_graph::source::resolve_media_type_and_charset_from_headers(
        &file.url,
        file.maybe_headers.as_ref(),
      );
    let specifier = file.url;
    let charset = maybe_charset.unwrap_or_else(|| {
      deno_media_type::encoding::detect_charset(&specifier, &file.source)
    });
    match deno_media_type::encoding::decode_arc_source(charset, file.source) {
      Ok(source) => Ok(TextDecodedFile {
        media_type,
        specifier,
        source,
      }),
      Err(err) => {
        Err(err).with_context(|| format!("Failed decoding \"{}\".", specifier))
      }
    }
  }
}

pub type CliFileFetcher = deno_resolver::file_fetcher::PermissionedFileFetcher<
  BlobStoreAdapter,
  CliSys,
  HttpClientAdapter,
>;
pub type CliDenoGraphLoader = deno_resolver::file_fetcher::DenoGraphLoader<
  BlobStoreAdapter,
  CliSys,
  HttpClientAdapter,
>;

pub struct CreateCliFileFetcherOptions {
  pub allow_remote: bool,
  pub cache_setting: CacheSetting,
  pub download_log_level: log::Level,
  pub progress_bar: Option<ProgressBar>,
}

#[allow(clippy::too_many_arguments)]
pub fn create_cli_file_fetcher(
  blob_store: Arc<BlobStore>,
  http_cache: GlobalOrLocalHttpCache<CliSys>,
  http_client_provider: Arc<HttpClientProvider>,
  sys: CliSys,
  options: CreateCliFileFetcherOptions,
) -> CliFileFetcher {
  CliFileFetcher::new(
    BlobStoreAdapter(blob_store),
    Arc::new(http_cache),
    HttpClientAdapter {
      http_client_provider: http_client_provider.clone(),
      download_log_level: options.download_log_level,
      progress_bar: options.progress_bar,
    },
    sys,
    PermissionedFileFetcherOptions {
      allow_remote: options.allow_remote,
      cache_setting: options.cache_setting,
    },
  )
}

#[derive(Debug)]
pub struct BlobStoreAdapter(Arc<BlobStore>);

#[async_trait::async_trait(?Send)]
impl deno_cache_dir::file_fetcher::BlobStore for BlobStoreAdapter {
  async fn get(&self, specifier: &Url) -> std::io::Result<Option<BlobData>> {
    let Some(blob) = self.0.get_object_url(specifier.clone()) else {
      return Ok(None);
    };
    Ok(Some(BlobData {
      media_type: blob.media_type.clone(),
      bytes: blob.read_all().await,
    }))
  }
}

#[derive(Debug)]
pub struct HttpClientAdapter {
  http_client_provider: Arc<HttpClientProvider>,
  download_log_level: log::Level,
  progress_bar: Option<ProgressBar>,
}

#[async_trait::async_trait(?Send)]
impl deno_cache_dir::file_fetcher::HttpClient for HttpClientAdapter {
  async fn send_no_follow(
    &self,
    url: &Url,
    headers: HeaderMap,
  ) -> Result<SendResponse, SendError> {
    async fn handle_request_or_server_error(
      retried: &mut bool,
      specifier: &Url,
      err_str: String,
    ) -> Result<(), ()> {
      // Retry once, and bail otherwise.
      if !*retried {
        *retried = true;
        log::debug!("Import '{}' failed: {}. Retrying...", specifier, err_str);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok(())
      } else {
        Err(())
      }
    }

    let mut maybe_progress_guard = None;
    if let Some(pb) = self.progress_bar.as_ref() {
      maybe_progress_guard = Some(pb.update(url.as_str()));
    } else {
      log::log!(
        self.download_log_level,
        "{} {}",
        deno_runtime::colors::green("Download"),
        url
      );
    }

    let mut retried = false; // retry intermittent failures
    loop {
      let response = match self
        .http_client_provider
        .get_or_create()
        .map_err(|err| SendError::Failed(err.into()))?
        .send(url, headers.clone())
        .await
      {
        Ok(response) => response,
        Err(crate::cli::http_util::SendError::Send(err)) => {
          if err.is_connect_error() {
            handle_request_or_server_error(&mut retried, url, err.to_string())
              .await
              .map_err(|()| SendError::Failed(err.into()))?;
            continue;
          } else {
            return Err(SendError::Failed(err.into()));
          }
        }
        Err(crate::cli::http_util::SendError::InvalidUri(err)) => {
          return Err(SendError::Failed(err.into()));
        }
      };
      if response.status() == StatusCode::NOT_MODIFIED {
        return Ok(SendResponse::NotModified);
      }

      if let Some(warning) = response.headers().get("X-Deno-Warning") {
        log::warn!(
          "{} {}",
          deno_runtime::colors::yellow("Warning"),
          warning.to_str().unwrap()
        );
      }

      if response.status().is_redirection() {
        return Ok(SendResponse::Redirect(response.into_parts().0.headers));
      }

      if response.status().is_server_error() {
        handle_request_or_server_error(
          &mut retried,
          url,
          response.status().to_string(),
        )
        .await
        .map_err(|()| SendError::StatusCode(response.status()))?;
      } else if response.status().is_client_error() {
        let err = if response.status() == StatusCode::NOT_FOUND {
          SendError::NotFound
        } else {
          SendError::StatusCode(response.status())
        };
        return Err(err);
      } else {
        let body_result = get_response_body_with_progress(
          response,
          maybe_progress_guard.as_ref(),
        )
        .await;

        match body_result {
          Ok((headers, body)) => {
            return Ok(SendResponse::Success(headers, body));
          }
          Err(err) => {
            handle_request_or_server_error(&mut retried, url, err.to_string())
              .await
              .map_err(|()| SendError::Failed(err.into()))?;
            continue;
          }
        }
      }
    }
  }
}