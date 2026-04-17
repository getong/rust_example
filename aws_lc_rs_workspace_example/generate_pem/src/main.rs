#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
  env,
  error::Error,
  ffi::{CStr, CString},
  fs, io,
  path::{Path, PathBuf},
  ptr,
};

use aws_lc_rs::{
  encoding::{AsDer, Pkcs8V1Der},
  rsa::{KeySize, PrivateDecryptingKey},
};
use aws_lc_sys::{
  ASN1_INTEGER_set_uint64, BIO, BIO_free, BIO_new_file, ERR_error_string_n, ERR_get_error,
  EVP_PKEY, EVP_PKEY_free, EVP_sha256, MBSTRING_ASC, NID_ext_key_usage, NID_key_usage,
  NID_subject_alt_name, PEM_write_bio_PrivateKey, PEM_write_bio_X509, X509, X509_EXTENSION,
  X509_EXTENSION_free, X509_NAME, X509_NAME_add_entry_by_txt, X509_NAME_free, X509_NAME_new,
  X509_add_ext, X509_free, X509_get_serialNumber, X509_getm_notAfter, X509_getm_notBefore,
  X509_gmtime_adj, X509_new, X509_set_issuer_name, X509_set_pubkey, X509_set_subject_name,
  X509_set_version, X509_sign, X509V3_CTX, X509V3_EXT_conf_nid, X509V3_set_ctx, d2i_AutoPrivateKey,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const DEFAULT_DAYS: u32 = 3650;
const SUBJECT_FIELDS: [(&str, &str); 6] = [
  ("C", "CN"),
  ("ST", "Shanghai"),
  ("L", "Shanghai"),
  ("O", "axum-local-dev"),
  ("OU", "websocket"),
  ("CN", "localhost"),
];
const SAN_VALUE: &str = "DNS:localhost,IP:127.0.0.1,IP:0:0:0:0:0:0:0:1";
const KEY_USAGE_VALUE: &str = "critical,digitalSignature,keyEncipherment";
const EXTENDED_KEY_USAGE_VALUE: &str = "serverAuth";

fn main() -> Result<()> {
  let cert_dir = output_dir();
  let cert_path = cert_dir.join("cert.pem");
  let key_path = cert_dir.join("key.pem");
  let days = env_days()?;

  fs::create_dir_all(&cert_dir)?;

  let private_key = PrivateDecryptingKey::generate(KeySize::Rsa2048)
    .map_err(|_| io::Error::other("failed to generate RSA key with aws-lc-rs"))?;
  let private_key_der = AsDer::<Pkcs8V1Der>::as_der(&private_key)
    .map_err(|_| io::Error::other("failed to serialize RSA key as PKCS#8"))?;

  let key = load_private_key(private_key_der.as_ref())?;
  let cert = build_self_signed_cert(key.as_ptr(), days)?;

  write_cert_pem(&cert_path, cert.as_ptr())?;
  write_private_key_pem(&key_path, key.as_ptr())?;

  #[cfg(unix)]
  fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;

  println!("Generated certificate:");
  println!("  cert: {}", cert_path.display());
  println!("  key : {}", key_path.display());
  println!();
  println!("Use with axum:");
  println!(
    "  TLS_CERT_PATH=\"{}\" TLS_KEY_PATH=\"{}\" cargo run",
    cert_path.display(),
    key_path.display()
  );

  Ok(())
}

fn output_dir() -> PathBuf {
  env::args_os()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("self_signed_certs"))
}

fn env_days() -> Result<u32> {
  match env::var("DAYS") {
    Ok(value) => Ok(value.parse()?),
    Err(env::VarError::NotPresent) => Ok(DEFAULT_DAYS),
    Err(err) => Err(Box::new(err)),
  }
}

fn load_private_key(pkcs8_der: &[u8]) -> Result<EvpPkey> {
  let mut der_ptr = pkcs8_der.as_ptr();
  let key = unsafe {
    d2i_AutoPrivateKey(
      ptr::null_mut(),
      &mut der_ptr,
      pkcs8_der
        .len()
        .try_into()
        .map_err(|_| io::Error::other("private key DER too large"))?,
    )
  };

  Ok(EvpPkey::new(key).map_err(|_| ffi_error("failed to parse generated PKCS#8 key"))?)
}

fn build_self_signed_cert(key: *mut EVP_PKEY, days: u32) -> Result<X509Cert> {
  let cert = X509Cert::new(unsafe { X509_new() })
    .map_err(|_| ffi_error("failed to allocate X509 certificate"))?;

  let validity_seconds = i64::from(days)
    .checked_mul(86_400)
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "DAYS is too large"))?;

  ensure_positive(
    unsafe { X509_set_version(cert.as_ptr(), 2) },
    "failed to set X509 version",
  )?;

  let serial = unsafe { X509_get_serialNumber(cert.as_ptr()) };
  if serial.is_null() {
    return Err(ffi_error("failed to access certificate serial number").into());
  }
  ensure_positive(
    unsafe { ASN1_INTEGER_set_uint64(serial, 1) },
    "failed to set certificate serial number",
  )?;

  let subject_name = X509Name::new(unsafe { X509_NAME_new() })
    .map_err(|_| ffi_error("failed to allocate certificate subject name"))?;

  for (field, value) in SUBJECT_FIELDS {
    add_name_entry(subject_name.as_ptr(), field, value)?;
  }

  ensure_positive(
    unsafe { X509_set_subject_name(cert.as_ptr(), subject_name.as_ptr()) },
    "failed to set certificate subject",
  )?;
  ensure_positive(
    unsafe { X509_set_issuer_name(cert.as_ptr(), subject_name.as_ptr()) },
    "failed to set certificate issuer",
  )?;

  let not_before = unsafe { X509_getm_notBefore(cert.as_ptr()) };
  let not_after = unsafe { X509_getm_notAfter(cert.as_ptr()) };
  if not_before.is_null() || not_after.is_null() {
    return Err(ffi_error("failed to access certificate validity timestamps").into());
  }

  if unsafe { X509_gmtime_adj(not_before, 0) }.is_null() {
    return Err(ffi_error("failed to set certificate notBefore").into());
  }
  if unsafe { X509_gmtime_adj(not_after, validity_seconds as _) }.is_null() {
    return Err(ffi_error("failed to set certificate notAfter").into());
  }

  ensure_positive(
    unsafe { X509_set_pubkey(cert.as_ptr(), key) },
    "failed to attach certificate public key",
  )?;

  add_extension(
    cert.as_ptr(),
    cert.as_ptr(),
    NID_subject_alt_name,
    SAN_VALUE,
  )?;
  add_extension(cert.as_ptr(), cert.as_ptr(), NID_key_usage, KEY_USAGE_VALUE)?;
  add_extension(
    cert.as_ptr(),
    cert.as_ptr(),
    NID_ext_key_usage,
    EXTENDED_KEY_USAGE_VALUE,
  )?;

  ensure_positive(
    unsafe { X509_sign(cert.as_ptr(), key, EVP_sha256()) },
    "failed to self-sign certificate",
  )?;

  Ok(cert)
}

fn add_name_entry(name: *mut aws_lc_sys::X509_NAME, field: &str, value: &str) -> Result<()> {
  let field = CString::new(field)?;
  let value = CString::new(value)?;
  ensure_positive(
    unsafe {
      X509_NAME_add_entry_by_txt(
        name,
        field.as_ptr(),
        MBSTRING_ASC,
        value.as_ptr().cast(),
        -1,
        -1,
        0,
      )
    },
    "failed to add distinguished name entry",
  )?;
  Ok(())
}

fn add_extension(issuer: *mut X509, subject: *mut X509, nid: i32, value: &str) -> Result<()> {
  let value = CString::new(value)?;
  let mut ctx = X509V3_CTX::default();
  unsafe {
    X509V3_set_ctx(&mut ctx, issuer, subject, ptr::null(), ptr::null(), 0);
  }

  let ext = unsafe { X509V3_EXT_conf_nid(ptr::null_mut(), &ctx, nid, value.as_ptr()) };
  let ext = X509Extension::new(ext).map_err(|_| ffi_error("failed to build X509 extension"))?;

  ensure_positive(
    unsafe { X509_add_ext(subject, ext.as_ptr(), -1) },
    "failed to attach X509 extension",
  )?;
  Ok(())
}

fn write_cert_pem(path: &Path, cert: *mut X509) -> Result<()> {
  let bio = open_file_bio(path)?;
  ensure_positive(
    unsafe { PEM_write_bio_X509(bio.as_ptr(), cert) },
    "failed to write certificate PEM",
  )?;
  Ok(())
}

fn write_private_key_pem(path: &Path, key: *mut EVP_PKEY) -> Result<()> {
  let bio = open_file_bio(path)?;
  ensure_positive(
    unsafe {
      PEM_write_bio_PrivateKey(
        bio.as_ptr(),
        key,
        ptr::null(),
        ptr::null_mut(),
        0,
        None,
        ptr::null_mut(),
      )
    },
    "failed to write private key PEM",
  )?;
  Ok(())
}

fn open_file_bio(path: &Path) -> Result<Bio> {
  let path = CString::new(path.to_string_lossy().into_owned())?;
  let mode = CString::new("w")?;
  let bio = unsafe { BIO_new_file(path.as_ptr(), mode.as_ptr()) };
  Bio::new(bio).map_err(|_| ffi_error("failed to open output file with BIO").into())
}

fn ensure_positive(status: i32, message: &'static str) -> Result<()> {
  if status > 0 {
    Ok(())
  } else {
    Err(ffi_error(message).into())
  }
}

fn ffi_error(message: &'static str) -> io::Error {
  let err = unsafe { ERR_get_error() };
  if err == 0 {
    io::Error::other(message)
  } else {
    let mut buf = [0u8; 256];
    unsafe { ERR_error_string_n(err, buf.as_mut_ptr().cast(), buf.len()) };
    let detail = unsafe { CStr::from_ptr(buf.as_ptr().cast()) }
      .to_string_lossy()
      .into_owned();
    io::Error::other(format!("{message}: {detail}"))
  }
}

struct EvpPkey(*mut EVP_PKEY);

impl EvpPkey {
  fn new(ptr: *mut EVP_PKEY) -> std::result::Result<Self, ()> {
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Self(ptr))
    }
  }

  fn as_ptr(&self) -> *mut EVP_PKEY {
    self.0
  }
}

impl Drop for EvpPkey {
  fn drop(&mut self) {
    unsafe { EVP_PKEY_free(self.0) };
  }
}

struct X509Cert(*mut X509);

impl X509Cert {
  fn new(ptr: *mut X509) -> std::result::Result<Self, ()> {
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Self(ptr))
    }
  }

  fn as_ptr(&self) -> *mut X509 {
    self.0
  }
}

impl Drop for X509Cert {
  fn drop(&mut self) {
    unsafe { X509_free(self.0) };
  }
}

struct X509Extension(*mut X509_EXTENSION);

impl X509Extension {
  fn new(ptr: *mut X509_EXTENSION) -> std::result::Result<Self, ()> {
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Self(ptr))
    }
  }

  fn as_ptr(&self) -> *mut X509_EXTENSION {
    self.0
  }
}

impl Drop for X509Extension {
  fn drop(&mut self) {
    unsafe { X509_EXTENSION_free(self.0) };
  }
}

struct Bio(*mut BIO);

impl Bio {
  fn new(ptr: *mut BIO) -> std::result::Result<Self, ()> {
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Self(ptr))
    }
  }

  fn as_ptr(&self) -> *mut BIO {
    self.0
  }
}

impl Drop for Bio {
  fn drop(&mut self) {
    unsafe {
      let _ = BIO_free(self.0);
    }
  }
}

struct X509Name(*mut X509_NAME);

impl X509Name {
  fn new(ptr: *mut X509_NAME) -> std::result::Result<Self, ()> {
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Self(ptr))
    }
  }

  fn as_ptr(&self) -> *mut X509_NAME {
    self.0
  }
}

impl Drop for X509Name {
  fn drop(&mut self) {
    unsafe { X509_NAME_free(self.0) };
  }
}
