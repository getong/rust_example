use std::{
  fs,
  io::Write,
  path::PathBuf,
  process::{Command, Output, Stdio},
  time::SystemTime,
};

struct DemoDir(PathBuf);

impl Drop for DemoDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

impl DemoDir {
  fn run(&self, bin: &str, args: &[&str], success: bool) {
    let stdin = if args.first() == Some(&"--keygen") {
      "\n"
    } else {
      ""
    };
    let output = self.run_stdin(bin, args, success, stdin);
    if stdin.is_empty() {
      assert!(!String::from_utf8_lossy(&output.stderr).contains("请输入密钥密码"));
    }
  }

  fn run_stdin(&self, bin: &str, args: &[&str], success: bool, stdin: &str) -> Output {
    let executable = match bin {
      "encrypt" => env!("CARGO_BIN_EXE_encrypt"),
      _ => env!("CARGO_BIN_EXE_decrypt"),
    };
    let mut child = Command::new(executable)
      .args(args)
      .current_dir(&self.0)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    child
      .stdin
      .take()
      .unwrap()
      .write_all(stdin.as_bytes())
      .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
      output.status.success(),
      success,
      "{args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    output
  }
}

#[test]
fn cli_roundtrip_and_reject_invalid_files() {
  let unique = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_nanos();
  let dir =
    DemoDir(std::env::temp_dir().join(format!("mlkem-demo-{}-{unique}", std::process::id())));
  fs::create_dir(&dir.0).unwrap();
  dir.run("decrypt", &["--keygen", "public", "private"], true);
  dir.run(
    "decrypt",
    &["--keygen", "other-public", "other-private"],
    true,
  );
  let original_key = fs::read(dir.0.join("private")).unwrap();
  dir.run("decrypt", &["--keygen", "public", "private"], false);
  assert_eq!(original_key, fs::read(dir.0.join("private")).unwrap());
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
      fs::metadata(dir.0.join("private"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777,
      0o600
    );
  }

  for (i, content) in [
    vec![],
    "中文文本\n".as_bytes().to_vec(),
    (0 .. 100_000).map(|n| n as u8).collect(),
  ]
  .iter()
  .enumerate()
  {
    let input = format!("input-{i}");
    let encrypted = format!("encrypted-{i}");
    let recovered = format!("recovered-{i}");
    fs::write(dir.0.join(&input), content).unwrap();
    dir.run("encrypt", &["public", &input, &encrypted], true);
    dir.run("decrypt", &["private", &encrypted, &recovered], true);
    assert_eq!(*content, fs::read(dir.0.join(&recovered)).unwrap());
    dir.run("decrypt", &["private", &encrypted, &recovered], false);
    assert_eq!(*content, fs::read(dir.0.join(&recovered)).unwrap());
  }

  dir.run("encrypt", &["public", "input-2", "second"], true);
  let valid = fs::read(dir.0.join("encrypted-2")).unwrap();
  assert_ne!(valid, fs::read(dir.0.join("second")).unwrap());
  dir.run(
    "decrypt",
    &["other-private", "encrypted-2", "rejected"],
    false,
  );
  assert!(!dir.0.join("rejected").exists());
  // 魔数、版本、算法、盐、KEM 密文、nonce、AES 密文和 tag 均不可篡改。
  for index in [0, 8, 9, 10, 42, 1610, 1622, valid.len() - 1] {
    let mut bad = valid.clone();
    bad[index] ^= 1;
    fs::write(dir.0.join("bad"), bad).unwrap();
    dir.run("decrypt", &["private", "bad", "rejected"], false);
    assert!(!dir.0.join("rejected").exists());
  }
  for length in [0, 10, 1621, 1637, valid.len() - 1] {
    fs::write(dir.0.join("bad"), &valid[.. length]).unwrap();
    dir.run("decrypt", &["private", "bad", "rejected"], false);
    assert!(!dir.0.join("rejected").exists());
  }
  let mut extended = valid.clone();
  extended.push(0);
  fs::write(dir.0.join("bad"), extended).unwrap();
  dir.run("decrypt", &["private", "bad", "rejected"], false);
  assert!(!dir.0.join("rejected").exists());
  dir.run("encrypt", &["public", "input-2", "input-2"], false);
  dir.run("encrypt", &["public", "input-2", "encrypted-2"], false);
  assert_eq!(valid, fs::read(dir.0.join("encrypted-2")).unwrap());
  fs::write(dir.0.join("invalid-key"), b"invalid").unwrap();
  dir.run("encrypt", &["invalid-key", "input-2", "rejected"], false);
  dir.run(
    "decrypt",
    &["invalid-key", "encrypted-2", "rejected"],
    false,
  );
  assert!(!dir.0.join("rejected").exists());
}

#[test]
fn password_protected_keys_require_passwords() {
  let unique = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_nanos();
  let dir =
    DemoDir(std::env::temp_dir().join(format!("mlkem-password-{}-{unique}", std::process::id())));
  fs::create_dir(&dir.0).unwrap();
  dir.run_stdin("decrypt", &["--keygen", "public", "private"], false, "");
  assert!(!dir.0.join("public").exists());
  assert!(!dir.0.join("private").exists());
  for input in ["first\nsecond\n", "first\n"] {
    dir.run_stdin("decrypt", &["--keygen", "public", "private"], false, input);
    assert!(!dir.0.join("private").exists());
    assert!(!dir.0.join("public").exists());
  }
  // Preserve spaces and Unicode; accept Windows CRLF as well as LF.
  let password = " 密码 with spaces ";
  dir.run_stdin(
    "decrypt",
    &["--keygen", "public", "private"],
    true,
    &format!("{password}\r\n{password}\n"),
  );
  fs::write(dir.0.join("input"), b"secret content").unwrap();
  assert_eq!(
    fs::metadata(dir.0.join("public")).unwrap().len(),
    encrypt_file::PUBLIC_KEY_LEN as u64
  );
  dir.run("encrypt", &["public", "input", "encrypted"], true);
  for input in ["wrong\n", "\n", ""] {
    let output = dir.run_stdin(
      "decrypt",
      &["private", "encrypted", "recovered"],
      false,
      input,
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("请输入密钥密码"));
    assert!(!dir.0.join("recovered").exists());
  }
  dir.run_stdin(
    "decrypt",
    &["private", "encrypted", "recovered"],
    true,
    &format!("{password}\n"),
  );
  assert_eq!(
    fs::read(dir.0.join("recovered")).unwrap(),
    b"secret content"
  );
  for (key, bin, input) in [("private", "decrypt", "encrypted")] {
    let valid = fs::read(dir.0.join(key)).unwrap();
    for index in [0, 8, 9, 10, 11, 15, 19, 23, 55, 67, valid.len() - 1] {
      let mut bad = valid.clone();
      bad[index] ^= 1;
      fs::write(dir.0.join("bad-key"), bad).unwrap();
      dir.run_stdin(
        bin,
        &["bad-key", input, "rejected"],
        false,
        &format!("{password}\n"),
      );
      assert!(!dir.0.join("rejected").exists());
    }
  }
}

#[test]
fn rejects_oversized_files_and_reads_legacy_private_key() {
  use std::num::NonZeroU32;

  use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
    pbkdf2,
  };
  let unique = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_nanos();
  let dir =
    DemoDir(std::env::temp_dir().join(format!("mlkem-legacy-{}-{unique}", std::process::id())));
  fs::create_dir(&dir.0).unwrap();
  dir.run("decrypt", &["--keygen", "public", "private"], true);
  let file = fs::File::create(dir.0.join("large")).unwrap();
  file.set_len(encrypt_file::MAX_PLAINTEXT_LEN + 1).unwrap();
  dir.run("encrypt", &["public", "large", "rejected"], false);
  file
    .set_len(encrypt_file::MAX_PLAINTEXT_LEN + encrypt_file::HEADER_LEN as u64 + 17)
    .unwrap();
  dir.run("decrypt", &["private", "large", "rejected"], false);
  assert!(!dir.0.join("rejected").exists());
  let mut header = b"ALCFKEY\0\x01\x01\x01".to_vec();
  header.extend_from_slice(&[7; 32]);
  header.extend_from_slice(&[8; 12]);
  let mut key = [0; 32];
  pbkdf2::derive(
    pbkdf2::PBKDF2_HMAC_SHA256,
    NonZeroU32::new(600_000).unwrap(),
    &[7; 32],
    b"legacy",
    &mut key,
  );
  let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key).unwrap());
  let mut private = fs::read(dir.0.join("private")).unwrap();
  key
    .seal_in_place_append_tag(
      Nonce::assume_unique_for_key([8; 12]),
      Aad::from(&header),
      &mut private,
    )
    .unwrap();
  header.extend_from_slice(&private);
  fs::write(dir.0.join("legacy"), &header).unwrap();
  fs::write(dir.0.join("input"), b"legacy roundtrip").unwrap();
  dir.run("encrypt", &["public", "input", "encrypted"], true);
  dir.run_stdin(
    "decrypt",
    &["legacy", "encrypted", "recovered"],
    true,
    "legacy\n",
  );
  assert_eq!(
    fs::read(dir.0.join("recovered")).unwrap(),
    b"legacy roundtrip"
  );
  header[10] = 0;
  fs::write(dir.0.join("legacy-public"), header).unwrap();
  dir.run("encrypt", &["legacy-public", "input", "rejected"], false);
}
