#![cfg(unix)]

use std::{
  fs,
  os::unix::fs::PermissionsExt,
  path::Path,
  process::{Command, Output},
};

use tempfile::TempDir;

fn executable(path: &Path, contents: &str) {
  fs::write(path, contents).unwrap();
  fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

struct Fixture {
  directory: TempDir,
}

impl Fixture {
  fn new() -> Self {
    let directory = TempDir::new().unwrap();
    let root = directory.path();
    fs::create_dir(root.join("scripts")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    for script in ["download.sh", "run.sh", "scripts/ollama-env.sh"] {
      fs::copy(source.join(script), root.join(script)).unwrap();
    }
    fs::write(root.join("installed"), "x/z-image-turbo\n").unwrap();
    executable(
      &root.join("bin/ollama"),
      r##"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOG"
case "$1" in
  run) if [ "${FAKE_UNSUPPORTED:-}" = 1 ]; then printf 'run MODEL\n'; else printf '%s\n' '--width --height --steps --seed'; fi ;;
  list) [ "${FAKE_SERVER_DOWN:-}" != 1 ] || exit 1; printf 'x/z-image-turbo\n' ;;
  --version) printf 'ollama version is %s\n' "${FAKE_VERSION:-0.15.0}" ;;
  show) grep -Fqx -- "$3" "$FAKE_INSTALLED" ;;
  pull) [ "${FAKE_PULL_FAILURE:-}" != 1 ] || exit 9; printf '%s\n' "$3" >> "$FAKE_INSTALLED" ;;
  *) exit 91 ;;
esac
"##,
    );
    executable(
      &root.join("bin/cargo"),
      "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CARGO_LOG\"\nprintf '%s\\n' \"$OLLAMA_HOST\" > \"$HOST_LOG\"\n",
    );
    Self { directory }
  }

  fn command(&self, script: &str) -> Command {
    let root = self.directory.path();
    let mut command = Command::new("bash");
    command
      .arg(root.join(script))
      .current_dir("/tmp")
      .env("OLLAMA_BIN", root.join("bin/ollama"))
      .env_remove("OLLAMA_MODEL")
      .env_remove("OLLAMA_IMAGE_MODEL")
      .env_remove("FAKE_PULL_FAILURE")
      .env_remove("FAKE_UNSUPPORTED")
      .env_remove("FAKE_SERVER_DOWN")
      .env_remove("FAKE_VERSION")
      .env("OLLAMA_HOST", "127.0.0.1:11435")
      .env("OLLAMA_MODELS", root.join("models"))
      .env("FAKE_INSTALLED", root.join("installed"))
      .env("FAKE_LOG", root.join("calls.log"))
      .env("CARGO_LOG", root.join("cargo.log"))
      .env("HOST_LOG", root.join("host.log"))
      .env(
        "PATH",
        format!(
          "{}:{}",
          root.join("bin").display(),
          std::env::var("PATH").unwrap()
        ),
      );
    command
  }

  fn log(&self, name: &str) -> String {
    fs::read_to_string(self.directory.path().join(name)).unwrap_or_default()
  }
}

fn success(output: &Output) {
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn download_reuses_installed_model_and_only_pulls_missing_models() {
  let fixture = Fixture::new();
  success(
    &fixture
      .command("download.sh")
      .args(["--text-model", "text:8b"])
      .output()
      .unwrap(),
  );
  let log = fixture.log("calls.log");
  assert!(!log.contains("pull -- x/z-image-turbo"));
  assert!(log.contains("pull -- text:8b"));
  assert!(fixture.log("installed").contains("text:8b"));
}

#[test]
fn refresh_pulls_existing_image_model() {
  let fixture = Fixture::new();
  success(
    &fixture
      .command("download.sh")
      .arg("--refresh")
      .output()
      .unwrap(),
  );
  assert!(fixture.log("calls.log").contains("pull -- x/z-image-turbo"));
}

#[test]
fn offline_never_pulls_and_missing_model_fails() {
  let fixture = Fixture::new();
  success(
    &fixture
      .command("download.sh")
      .arg("--offline")
      .output()
      .unwrap(),
  );
  let output = fixture
    .command("download.sh")
    .args(["--offline", "--model", "missing:latest"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(!fixture.log("calls.log").contains("pull"));
}

#[test]
fn pull_failure_is_not_reported_as_success() {
  let fixture = Fixture::new();
  let output = fixture
    .command("download.sh")
    .env("FAKE_PULL_FAILURE", "1")
    .args(["--model", "missing:latest"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(!String::from_utf8_lossy(&output.stdout).contains("准备完成"));
}

#[test]
fn unsupported_runtime_fails_before_pulling_or_running() {
  for script in ["download.sh", "run.sh"] {
    let fixture = Fixture::new();
    let output = fixture
      .command(script)
      .env("FAKE_UNSUPPORTED", "1")
      .output()
      .unwrap();
    assert!(!output.status.success());
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("不支持图像生成"),
      "{script}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.log("calls.log").contains("pull"));
    assert!(fixture.log("cargo.log").is_empty());
  }
}

#[test]
fn run_forwards_literal_arguments_and_custom_host_without_python() {
  let fixture = Fixture::new();
  let output = fixture
    .command("run.sh")
    .args([
      "--prompt",
      "--help",
      "--host",
      "127.0.0.1:19435",
      "--steps=9",
    ])
    .output()
    .unwrap();
  success(&output);
  let log = fixture.log("cargo.log");
  assert!(log.contains("--prompt\n--help\n--host\n127.0.0.1:19435\n--steps=9\n"));
  assert!(!log.contains("--python"));
  assert!(!log.contains("--model-dir"));
  assert_eq!(fixture.log("host.log"), "127.0.0.1:19435\n");
}

#[test]
fn dry_run_and_help_do_not_require_installed_runtime() {
  let fixture = Fixture::new();
  success(
    &fixture
      .command("run.sh")
      .env("OLLAMA_BIN", "/not/installed")
      .args(["--prompt", "horse", "--dry-run"])
      .output()
      .unwrap(),
  );
  assert!(fixture.log("calls.log").is_empty());
  for script in ["download.sh", "run.sh"] {
    success(
      &fixture
        .command(script)
        .env("OLLAMA_BIN", "/not/installed")
        .arg("--help")
        .output()
        .unwrap(),
    );
  }
}

#[test]
fn rejects_obsolete_options_invalid_models_and_offline_refresh_conflict() {
  for arguments in [
    vec!["--reuse-dir", "/old/cache"],
    vec!["--offline", "--refresh"],
    vec!["--model"],
    vec!["--model", "bad model"],
  ] {
    let fixture = Fixture::new();
    let output = fixture
      .command("download.sh")
      .args(&arguments)
      .output()
      .unwrap();
    assert!(!output.status.success(), "accepted {arguments:?}");
    assert!(fixture.log("calls.log").is_empty());
  }
}

#[test]
fn managed_runtime_rejects_wrong_server_version_without_pulling() {
  let fixture = Fixture::new();
  let runtime = fixture.directory.path().join(".tools/ollama-v0.15.0");
  fs::create_dir_all(&runtime).unwrap();
  fs::copy(
    fixture.directory.path().join("bin/ollama"),
    runtime.join("ollama"),
  )
  .unwrap();
  let output = fixture
    .command("download.sh")
    .env_remove("OLLAMA_BIN")
    .env("FAKE_VERSION", "0.40.16")
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("不是配套"));
  assert!(!fixture.log("calls.log").contains("pull"));
}

#[test]
fn custom_runtime_does_not_start_or_replace_external_service() {
  let fixture = Fixture::new();
  let output = fixture
    .command("download.sh")
    .env("FAKE_SERVER_DOWN", "1")
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("不会启动或替换"));
  assert!(!fixture.log("calls.log").contains("serve"));
  assert!(!fixture.log("calls.log").contains("pull"));
}
