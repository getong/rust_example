//! Reusable mTLS client for the Agones Allocator gRPC API.

use std::{path::PathBuf, sync::Arc, time::Duration};

use thiserror::Error;
use tokio::{sync::Semaphore, time::Instant};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

pub mod allocation {
  tonic::include_proto!("allocation");
}

use allocation::{
  AllocationRequest, AllocationResponse, allocation_service_client::AllocationServiceClient,
};

/// TLS credentials, deadlines, and shared concurrency limit for an allocator endpoint.
#[derive(Clone, Debug)]
pub struct AllocatorConfig {
  pub endpoint: String,
  pub ca_cert: PathBuf,
  pub client_cert: PathBuf,
  pub client_key: PathBuf,
  pub server_name: Option<String>,
  pub connect_timeout: Duration,
  pub request_timeout: Duration,
  pub max_in_flight: usize,
}

impl AllocatorConfig {
  pub fn new(
    endpoint: impl Into<String>,
    ca_cert: impl Into<PathBuf>,
    client_cert: impl Into<PathBuf>,
    client_key: impl Into<PathBuf>,
  ) -> Self {
    Self {
      endpoint: endpoint.into(),
      ca_cert: ca_cert.into(),
      client_cert: client_cert.into(),
      client_key: client_key.into(),
      server_name: None,
      connect_timeout: Duration::from_secs(5),
      request_timeout: Duration::from_secs(5),
      max_in_flight: 32,
    }
  }
}

#[derive(Debug, Error)]
pub enum AllocatorError {
  #[error("invalid allocator configuration: {0}")]
  InvalidConfig(String),
  #[error("failed to read credential file {path}: {source}")]
  Credential {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },
  #[error("allocator transport failed: {0}")]
  Transport(#[from] tonic::transport::Error),
  #[error("allocator RPC failed: {0}")]
  Rpc(#[source] Box<tonic::Status>),
  #[error("allocation deadline exceeded after {0:?}; the allocation outcome may be unknown")]
  DeadlineExceeded(Duration),
  #[error("allocator concurrency limiter is closed")]
  Closed,
}

/// Clones reuse the HTTP/2 channel and share the same in-flight limit.
#[derive(Clone)]
pub struct AllocatorClient {
  client: AllocationServiceClient<Channel>,
  in_flight: Arc<Semaphore>,
  request_timeout: Duration,
}

impl AllocatorClient {
  /// Connect using a trusted server CA and an authorized client identity.
  ///
  /// # Errors
  /// Returns an error for invalid configuration, unreadable credentials, or TLS/transport failure.
  pub async fn connect(config: AllocatorConfig) -> Result<Self, AllocatorError> {
    if config.max_in_flight == 0 || config.max_in_flight > Semaphore::MAX_PERMITS {
      return Err(AllocatorError::InvalidConfig(format!(
        "max_in_flight must be between 1 and {}",
        Semaphore::MAX_PERMITS
      )));
    }
    for (name, timeout) in [
      ("connect_timeout", config.connect_timeout),
      ("request_timeout", config.request_timeout),
    ] {
      if timeout.is_zero() || Instant::now().checked_add(timeout).is_none() {
        return Err(AllocatorError::InvalidConfig(format!(
          "{name} must be positive and fit within the monotonic clock range"
        )));
      }
    }
    if config
      .server_name
      .as_ref()
      .is_some_and(|name| name.trim().is_empty())
    {
      return Err(AllocatorError::InvalidConfig(
        "server_name must not be empty".into(),
      ));
    }

    let endpoint = Endpoint::from_shared(config.endpoint)?;
    if endpoint.uri().scheme_str() != Some("https")
      || endpoint.uri().host().is_none()
      || !matches!(endpoint.uri().path(), "" | "/")
      || endpoint.uri().query().is_some()
      || endpoint
        .uri()
        .authority()
        .is_some_and(|authority| authority.as_str().contains('@'))
    {
      return Err(AllocatorError::InvalidConfig(
        "endpoint must be an https origin such as https://allocator.example.com:443".into(),
      ));
    }

    let ca = read_credential(config.ca_cert).await?;
    let cert = read_credential(config.client_cert).await?;
    let key = read_credential(config.client_key).await?;
    let mut tls = ClientTlsConfig::new()
      .ca_certificate(Certificate::from_pem(ca))
      .identity(Identity::from_pem(cert, key));
    if let Some(server_name) = config.server_name {
      tls = tls.domain_name(server_name);
    }

    let channel = endpoint
      .connect_timeout(config.connect_timeout)
      .tcp_nodelay(true)
      .tls_config(tls)?
      .connect()
      .await?;

    Ok(Self {
      client: AllocationServiceClient::new(channel),
      in_flight: Arc::new(Semaphore::new(config.max_in_flight)),
      request_timeout: config.request_timeout,
    })
  }

  /// Allocate once, with a deadline that includes waiting for a concurrency permit.
  ///
  /// No automatic retries are performed: an interrupted RPC may already have allocated a server.
  /// The generated request exposes selectors, metadata, multi-cluster policy, counters, and lists.
  ///
  /// # Errors
  /// Returns an error for an empty namespace, deadline expiry, or a gRPC status from Agones.
  pub async fn allocate(
    &self,
    request: AllocationRequest,
  ) -> Result<AllocationResponse, AllocatorError> {
    if request.namespace.trim().is_empty() {
      return Err(AllocatorError::InvalidConfig(
        "allocation namespace must not be empty".into(),
      ));
    }

    let started = Instant::now();
    let allocation = async {
      let _permit = self
        .in_flight
        .acquire()
        .await
        .map_err(|_| AllocatorError::Closed)?;
      let remaining = self
        .request_timeout
        .checked_sub(started.elapsed())
        .filter(|timeout| !timeout.is_zero())
        .ok_or(AllocatorError::DeadlineExceeded(self.request_timeout))?;
      let mut request = tonic::Request::new(request);
      request.set_timeout(remaining);

      let mut client = self.client.clone();
      client
        .allocate(request)
        .await
        .map(tonic::Response::into_inner)
        .map_err(|status| AllocatorError::Rpc(Box::new(status)))
    };

    tokio::time::timeout(self.request_timeout, allocation)
      .await
      .map_err(|_| AllocatorError::DeadlineExceeded(self.request_timeout))?
  }
}

async fn read_credential(path: PathBuf) -> Result<Vec<u8>, AllocatorError> {
  tokio::fs::read(&path)
    .await
    .map_err(|source| AllocatorError::Credential { path, source })
}
