use std::{
  fs,
  io::Write,
  path::PathBuf,
  process::{Command, Output, Stdio},
  time::{SystemTime, UNIX_EPOCH},
};

use encrypt_file::{keys::fingerprint_hex, *};

struct DemoDir(PathBuf);
impl DemoDir {
  fn new() -> Self {
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path =
      std::env::temp_dir().join(format!("encrypt-cli-{}-{stamp}-{id}", std::process::id()));
    fs::create_dir(&path).unwrap();
    Self(path)
  }
  fn run(&self, bin: &str, args: &[&str], input: &str, success: bool) -> Output {
    let exe = if bin == "encrypt" {
      env!("CARGO_BIN_EXE_encrypt")
    } else {
      env!("CARGO_BIN_EXE_decrypt")
    };
    let mut child = Command::new(exe)
      .args(args)
      .current_dir(&self.0)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    // A rejecting CLI can exit before consuming input.
    let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
    let output = child.wait_with_output().unwrap();
    assert_eq!(
      output.status.success(),
      success,
      "{args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    output
  }
  fn plain_keys(&self) {
    self.run(
      "decrypt",
      &["--keygen", "--no-password", "public", "private"],
      "",
      true,
    );
    self.run(
      "decrypt",
      &["--sign-keygen", "--no-password", "sender", "signer"],
      "",
      true,
    );
  }
}
impl Drop for DemoDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[test]
fn explicit_password_policy_and_signed_cli_roundtrip() {
  let dir = DemoDir::new();
  for input in ["", "\n", "first\nsecond\n", "first\n"] {
    dir.run("decrypt", &["--keygen", "public", "private"], input, false);
    assert!(!dir.0.join("private").exists());
    assert!(!dir.0.join("public").exists());
  }
  let password = " 密码 with spaces ";
  let confirm = format!("{password}\r\n{password}\n");
  dir.run(
    "decrypt",
    &["--keygen", "public", "private"],
    &confirm,
    true,
  );
  dir.run(
    "decrypt",
    &["--sign-keygen", "sender", "signer"],
    &confirm,
    true,
  );
  fs::write(dir.0.join("input"), b"secret").unwrap();
  for input in ["wrong\n", "\n", ""] {
    dir.run(
      "encrypt",
      &["--sign-key", "signer", "public", "input", "encrypted"],
      input,
      false,
    );
    assert!(!dir.0.join("encrypted").exists());
  }
  dir.run(
    "encrypt",
    &["--sign-key", "signer", "public", "input", "encrypted"],
    &format!("{password}\n{password}\n"),
    true,
  );
  for input in ["wrong\n", "\n", ""] {
    dir.run(
      "decrypt",
      &[
        "--verify-key",
        "sender",
        "private",
        "encrypted",
        "recovered",
      ],
      input,
      false,
    );
    assert!(!dir.0.join("recovered").exists());
  }
  dir.run(
    "decrypt",
    &[
      "--verify-key",
      "sender",
      "private",
      "encrypted",
      "recovered",
    ],
    &format!("{password}\n"),
    true,
  );
  assert_eq!(fs::read(dir.0.join("recovered")).unwrap(), b"secret");
  let fingerprint = dir.run("decrypt", &["--fingerprint", "sign", "sender"], "", true);
  assert!(
    String::from_utf8_lossy(&fingerprint.stdout)
      .contains(&fingerprint_hex(&fs::read(dir.0.join("sender")).unwrap()))
  );
  dir.run(
    "decrypt",
    &["--allow-unsigned-legacy", "private", "encrypted", "bypass"],
    "",
    false,
  );
  assert!(!dir.0.join("bypass").exists());
}

#[test]
fn no_overwrite_and_reject_untrusted_inputs_before_password_prompt() {
  let dir = DemoDir::new();
  dir.plain_keys();
  let original = fs::read(dir.0.join("private")).unwrap();
  dir.run(
    "decrypt",
    &["--keygen", "--no-password", "public", "private"],
    "",
    false,
  );
  assert_eq!(fs::read(dir.0.join("private")).unwrap(), original);
  fs::write(dir.0.join("input"), b"test").unwrap();
  dir.run(
    "encrypt",
    &["--sign-key", "signer", "public", "input", "encrypted"],
    "",
    true,
  );
  dir.run(
    "decrypt",
    &[
      "--verify-key",
      "sender",
      "private",
      "encrypted",
      "recovered",
    ],
    "",
    true,
  );
  dir.run(
    "decrypt",
    &[
      "--verify-key",
      "sender",
      "private",
      "encrypted",
      "recovered",
    ],
    "",
    false,
  );
  assert_eq!(fs::read(dir.0.join("recovered")).unwrap(), b"test");
  dir.run(
    "encrypt",
    &["--sign-key", "signer", "public", "input", "input"],
    "",
    false,
  );
  assert_eq!(fs::read(dir.0.join("input")).unwrap(), b"test");
  let mut ciphertext = fs::read(dir.0.join("encrypted")).unwrap();
  let end = ciphertext.len();
  ciphertext[end - 1] ^= 1;
  fs::write(dir.0.join("bad"), &ciphertext).unwrap();
  // NoPassword errors if called; signature verification must fail first even with a missing private
  // key.
  let verifier = fs::read(dir.0.join("sender")).unwrap();
  let error = decrypt_file(
    &dir.0.join("missing-private"),
    &dir.0.join("bad"),
    &dir.0.join("rejected"),
    Verification::Trusted(&verifier),
    &mut NoPassword,
  )
  .unwrap_err();
  assert!(matches!(error, Error::SignatureInvalid));
  assert!(!dir.0.join("rejected").exists());
  let oversized = fs::File::create(dir.0.join("large")).unwrap();
  oversized.set_len(MAX_PLAINTEXT_LEN + 1).unwrap();
  dir.run(
    "encrypt",
    &["--sign-key", "signer", "public", "large", "rejected"],
    "",
    false,
  );
  assert!(!dir.0.join("rejected").exists());
  oversized.set_len(MAX_ENCRYPTED_LEN + 1).unwrap();
  dir.run(
    "decrypt",
    &["--verify-key", "sender", "private", "large", "rejected"],
    "",
    false,
  );
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    for name in ["private", "signer", "recovered"] {
      assert_eq!(
        fs::metadata(dir.0.join(name)).unwrap().permissions().mode() & 0o777,
        0o600
      );
    }
  }
  #[cfg(windows)]
  {
    let status=Command::new("powershell").args(["-NoProfile","-Command",r#"
      $acl = Get-Acl -LiteralPath $env:ENCRYPT_TEST_KEY
      if (-not $acl.AreAccessRulesProtected) { exit 1 }
      $rules = $acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier])
      if ($rules.Count -ne 1 -or $rules[0].IdentityReference.Value -ne 'S-1-3-4' -or
          $rules[0].AccessControlType -ne 'Allow' -or $rules[0].IsInherited -or
          $rules[0].FileSystemRights -ne [System.Security.AccessControl.FileSystemRights]::FullControl) { exit 2 }
    "#]).env("ENCRYPT_TEST_KEY",dir.0.join("private")).status().unwrap();
    assert!(status.success());
  }
}

#[test]
fn legacy_cli_requires_explicit_opt_in() {
  let dir = DemoDir::new();
  fs::write(dir.0.join("private"), include_bytes!("fixtures/v1.private")).unwrap();
  fs::write(
    dir.0.join("encrypted"),
    include_bytes!("fixtures/v1.encrypted"),
  )
  .unwrap();
  dir.run("decrypt", &["private", "encrypted", "recovered"], "", false);
  dir.run(
    "decrypt",
    &[
      "--allow-unsigned-legacy",
      "private",
      "encrypted",
      "recovered",
    ],
    "",
    true,
  );
  assert_eq!(
    fs::read(dir.0.join("recovered")).unwrap(),
    include_bytes!("fixtures/v1.plaintext")
  );
}

#[test]
fn library_password_provider_and_public_unprotected_option() {
  let dir = DemoDir::new();
  generate_keys(
    &dir.0.join("public"),
    &dir.0.join("private"),
    Protection::Password("password"),
    false,
  )
  .unwrap();
  let public = read_key(&dir.0.join("public"), KeyKind::KemPublic, &mut NoPassword).unwrap();
  let mut provider = FixedPassword(zeroize::Zeroizing::new("password".to_owned()));
  let private = read_key(&dir.0.join("private"), KeyKind::KemPrivate, &mut provider).unwrap();
  let key = aws_lc_rs::kem::DecapsulationKey::new(&aws_lc_rs::kem::ML_KEM_1024, &private).unwrap();
  let encapsulation =
    aws_lc_rs::kem::EncapsulationKey::new(&aws_lc_rs::kem::ML_KEM_1024, &public).unwrap();
  let (ciphertext, secret) = encapsulation.encapsulate().unwrap();
  assert_eq!(
    key.decapsulate(ciphertext).unwrap().as_ref(),
    secret.as_ref()
  );
}

#[test]
fn password_byte_limit_and_crlf_are_consistent() {
  let dir = DemoDir::new();
  let overlong = "x".repeat(1025);
  dir.run(
    "decrypt",
    &["--keygen", "--public-unprotected", "public", "private"],
    &format!("{overlong}\n"),
    false,
  );
  assert!(!dir.0.join("private").exists());
  let password = "x".repeat(1024);
  dir.run(
    "decrypt",
    &["--keygen", "--public-unprotected", "public", "private"],
    &format!("{password}\r\n{password}\r\n"),
    true,
  );
  let mut provider = FixedPassword(zeroize::Zeroizing::new(password));
  assert_eq!(
    read_key(&dir.0.join("private"), KeyKind::KemPrivate, &mut provider)
      .unwrap()
      .len(),
    PRIVATE_KEY_LEN
  );
  assert_eq!(
    read_key(&dir.0.join("public"), KeyKind::KemPublic, &mut NoPassword)
      .unwrap()
      .len(),
    PUBLIC_KEY_LEN
  );
}
