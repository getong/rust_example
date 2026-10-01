use std::{fs, path::PathBuf, process::Command, time::SystemTime};

struct DemoDir(PathBuf);

impl Drop for DemoDir {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

impl DemoDir {
  fn run(&self, bin: &str, args: &[&str], success: bool) {
    let executable = match bin {
      "encrypt" => env!("CARGO_BIN_EXE_encrypt"),
      _ => env!("CARGO_BIN_EXE_decrypt"),
    };
    let output = Command::new(executable)
      .args(args)
      .current_dir(&self.0)
      .output()
      .unwrap();
    assert_eq!(
      output.status.success(),
      success,
      "{args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
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
