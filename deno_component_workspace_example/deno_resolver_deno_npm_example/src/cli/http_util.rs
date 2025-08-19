use std::sync::Arc;
use deno_error::JsErrorBox;
use reqwest::Client;
use url::Url;
use http::{HeaderName, HeaderValue};

#[derive(Debug)]
pub struct HttpClientProvider {
    client: Client,
}

impl HttpClientProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn get_or_create(&self) -> Result<HttpClient, JsErrorBox> {
        Ok(HttpClient {
            client: self.client.clone(),
        })
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: Client,
}

#[derive(Debug)]
pub enum HttpClientResponse {
    Success {
        headers: http::HeaderMap,
        body: Vec<u8>,
    },
    NotFound,
    NotModified,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadErrorKind {
    #[error("Fetch error: {0}")]
    Fetch(String),
    #[error("URL parse error")]
    UrlParse,
    #[error("Bad response")]
    BadResponse(BadResponseError),
    #[error("Too many redirects")]
    TooManyRedirects,
    #[error("Not found")]
    NotFound,
    #[error("Unhandled not modified")]
    UnhandledNotModified,
    #[error("Other error: {0}")]
    Other(String),
    #[error("Redirect header parse error")]
    RedirectHeaderParse,
    #[error("HTTP parse error")]
    HttpParse,
    #[error("JSON error")]
    Json,
    #[error("ToString error")]
    ToStr,
}

#[derive(Debug)]
pub struct BadResponseError {
    pub status_code: http::StatusCode,
    pub response_text: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct DownloadError(pub Box<DownloadErrorKind>);

impl DownloadError {
    pub fn as_kind(&self) -> &DownloadErrorKind {
        &self.0
    }
}

impl From<DownloadErrorKind> for DownloadError {
    fn from(kind: DownloadErrorKind) -> Self {
        DownloadError(Box::new(kind))
    }
}

impl From<DownloadError> for JsErrorBox {
    fn from(err: DownloadError) -> Self {
        JsErrorBox::generic(err.to_string())
    }
}

pub trait IntoBox<T> {
    fn into_box(self) -> T;
}

impl IntoBox<DownloadError> for DownloadErrorKind {
    fn into_box(self) -> DownloadError {
        DownloadError(Box::new(self))
    }
}

// Simple progress guard placeholder
pub struct UpdateGuard;

impl HttpClient {
    pub async fn download_with_progress_and_retries(
        &self,
        url: Url,
        headers: &http::HeaderMap,
        _guard: &UpdateGuard,
    ) -> Result<HttpClientResponse, DownloadError> {
        let mut request = self.client.get(url.as_str());
        
        // Add headers
        for (key, value) in headers {
            if let Ok(value_str) = value.to_str() {
                request = request.header(key.as_str(), value_str);
            }
        }
        
        let response = request.send().await
            .map_err(|e| DownloadErrorKind::Fetch(e.to_string()).into_box())?;
        
        let status = response.status();
        
        if status == 404 {
            return Ok(HttpClientResponse::NotFound);
        } else if status == 304 {
            return Ok(HttpClientResponse::NotModified);
        } else if !status.is_success() {
            return Err(DownloadErrorKind::BadResponse(BadResponseError {
                status_code: http::StatusCode::from_u16(status.as_u16()).unwrap(),
                response_text: None,
            }).into_box());
        }
        
        // Convert reqwest headers to http headers
        let mut http_headers = http::HeaderMap::new();
        for (key, value) in response.headers() {
            if let Ok(name) = http::HeaderName::from_bytes(key.as_str().as_bytes()) {
                if let Ok(value) = http::HeaderValue::from_bytes(value.as_bytes()) {
                    http_headers.insert(name, value);
                }
            }
        }
        
        let body = response.bytes().await
            .map_err(|e| DownloadErrorKind::Fetch(e.to_string()).into_box())?
            .to_vec();
        
        Ok(HttpClientResponse::Success { headers: http_headers, body })
    }
}