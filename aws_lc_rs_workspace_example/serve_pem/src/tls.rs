use std::{env, io};

use axum_server::tls_rustls::RustlsConfig;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_RSA_SHA256};
use rustls_pki_types::PrivatePkcs8KeyDer;

use crate::CryptoState;

const DEFAULT_TLS_CERT_HOSTS: &[&str] = &["localhost", "127.0.0.1", "::1"];
const TLS_CERT_HOSTS_ENV: &str = "TLS_CERT_HOSTS";

fn tls_cert_hosts() -> Vec<String> {
  let configured_hosts = env::var(TLS_CERT_HOSTS_ENV).ok().and_then(|value| {
    let hosts = value
      .split(',')
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .map(ToOwned::to_owned)
      .collect::<Vec<_>>();
    (!hosts.is_empty()).then_some(hosts)
  });

  configured_hosts.unwrap_or_else(|| {
    DEFAULT_TLS_CERT_HOSTS
      .iter()
      .map(|value| (*value).to_owned())
      .collect()
  })
}

pub(crate) async fn rustls_config_from_crypto_state(
  state: &CryptoState,
) -> Result<RustlsConfig, io::Error> {
  let private_key = PrivatePkcs8KeyDer::from(state.private_key_der.as_ref().clone());
  let signing_key = KeyPair::from_pkcs8_der_and_sign_algo(&private_key, &PKCS_RSA_SHA256)
    .map_err(|error| io::Error::other(format!("failed to prepare TLS signing key: {error}")))?;

  let mut params = CertificateParams::new(tls_cert_hosts()).map_err(|error| {
    io::Error::other(format!("failed to build TLS certificate params: {error}"))
  })?;
  let mut distinguished_name = DistinguishedName::new();
  distinguished_name.push(DnType::CommonName, "serve_pem");
  params.distinguished_name = distinguished_name;

  let certificate = params.self_signed(&signing_key).map_err(|error| {
    io::Error::other(format!(
      "failed to build self-signed TLS certificate: {error}"
    ))
  })?;

  RustlsConfig::from_der(
    vec![certificate.der().as_ref().to_vec()],
    state.private_key_der.as_ref().clone(),
  )
  .await
}
