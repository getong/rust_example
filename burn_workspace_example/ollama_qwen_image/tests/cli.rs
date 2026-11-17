#![cfg(unix)]

use std::{
  fs,
  io::Write,
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
  process::{Command, Output, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

fn executable(path: &Path, contents: &str) {
  fs::write(path, contents).unwrap();
  fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn fake_ollama(directory: &Path) -> PathBuf {
  let path = directory.join("fake-ollama");
  executable(
    &path,
    r##"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_CALLS"
case "$1" in
  list) printf 'NAME ID SIZE MODIFIED\nx/z-image-turbo:latest fake 6GB now\n'; exit 0 ;;
  show)
    if [ "${FAKE_MODE:-}" = missing ]; then printf 'model not found\n' >&2; exit 1; fi
    printf 'Capabilities: image\n'; exit 0 ;;
  run)
    if [ "$2" = --help ]; then
      if [ "${FAKE_MODE:-}" = unsupported ]; then printf 'run MODEL\n'; else printf '%s\n' '--width --height --steps --seed'; fi
      exit 0
    fi
    if [ "$2" = --format ]; then
      printf '%s\n' "${FAKE_RESPONSE:-{\"prompt\":\"A running horse\"}}"; exit 0
    fi
    case "${FAKE_MODE:-}" in
      failure) printf 'image generation failed\n' >&2; exit 7 ;;
      timeout) exec sleep 5 ;;
      noimage) exit 0 ;;
      corrupt) printf 'not a PNG' > generated.png; exit 0 ;;
      multiple) cp "$FAKE_FIXTURE" extra.png ;;
      race) printf 'created by someone else' > "$FAKE_RACE_OUTPUT" ;;
    esac
    cp "$FAKE_FIXTURE" generated.png
    printf 'Image saved to: generated.png\n'
    ;;
  *) exit 90 ;;
esac
"##,
  );
  path
}

fn command(directory: &Path) -> Command {
  let fake = fake_ollama(directory);
  let fixture = directory.join("fixture.png");
  image::RgbImage::new(64, 64).save(&fixture).unwrap();
  let mut command = Command::new(env!("CARGO_BIN_EXE_ollama_qwen_image"));
  command
    .current_dir(directory)
    .env_remove("OLLAMA_MODEL")
    .env_remove("OLLAMA_IMAGE_MODEL")
    .env_remove("FAKE_MODE")
    .env_remove("FAKE_RESPONSE")
    .env("OLLAMA_BIN", fake)
    .env("OLLAMA_HOST", "127.0.0.1:11435")
    .env("FAKE_CALLS", directory.join("calls.log"))
    .env("FAKE_FIXTURE", fixture)
    .args(["--width", "64", "--height", "64"]);
  command
}

fn success(output: &Output) {
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
}

fn saved(directory: &Path, name: &str) -> Value {
  serde_json::from_slice(&fs::read(directory.join("output-0001.artifacts").join(name)).unwrap())
    .unwrap()
}

#[test]
fn dry_run_has_no_runtime_or_model_dependency() {
  let directory = TempDir::new().unwrap();
  let prompt = "画一匹马；$(touch unexpected)";
  let output = command(directory.path())
    .args([
      "--prompt",
      prompt,
      "--dry-run",
      "--ollama",
      "/not/installed",
    ])
    .output()
    .unwrap();
  success(&output);
  assert_eq!(saved(directory.path(), "generation.json")["prompt"], prompt);
  assert_eq!(
    saved(directory.path(), "generation.json")["backend"],
    "ollama-cli"
  );
  assert_eq!(
    saved(directory.path(), "generation.json")["model"],
    "x/z-image-turbo"
  );
  assert!(!directory.path().join("calls.log").exists());
  assert!(!directory.path().join("unexpected").exists());
  assert!(!directory.path().join("output-0001.png").exists());
}

#[test]
fn validates_options_before_creating_artifacts() {
  for arguments in [
    vec!["--prompt", " "],
    vec!["--prompt", "test", "--width", "96"],
    vec!["--prompt", "test", "--height", "0"],
    vec!["--prompt", "test", "--steps", "1001"],
    vec!["--prompt", "test", "--seed", "-1"],
    vec!["--prompt", "test", "--output", "test.jpg"],
    vec!["--output", "test.png"],
    vec!["--prompt", "test", "--image-model", "bad model"],
    vec!["--prompt", "test", "--python", "python3"],
    vec!["--prompt", "test", "--device", "mps"],
    vec![
      "--prompt",
      "test",
      "--skip-ollama",
      "--response",
      "response.json",
    ],
  ] {
    let directory = TempDir::new().unwrap();
    let output = command(directory.path()).args(&arguments).output().unwrap();
    assert!(!output.status.success(), "accepted {arguments:?}");
    assert!(!directory.path().join("output-0001.artifacts").exists());
  }
}

#[test]
fn generates_png_via_cli_with_flags_and_literal_prompt() {
  let directory = TempDir::new().unwrap();
  let output = command(directory.path())
    .args([
      "--prompt",
      "--help;$(touch unexpected)",
      "--steps",
      "9",
      "--seed",
      "73",
    ])
    .output()
    .unwrap();
  success(&output);
  let calls = fs::read_to_string(directory.path().join("calls.log")).unwrap();
  assert!(calls.contains(
    "run --width 64 --height 64 --steps 9 --seed 73 -- x/z-image-turbo --help;$(touch unexpected)"
  ));
  assert!(!calls.contains("pull"));
  assert!(!directory.path().join("unexpected").exists());
  assert_eq!(saved(directory.path(), "result.json")["width"], 64);
  assert!(directory.path().join("output-0001.png").exists());
}

#[test]
fn rejects_unsupported_runtime_or_missing_model_before_inference() {
  for (mode, expected) in [
    ("unsupported", "不支持图像生成"),
    ("missing", "./download.sh"),
  ] {
    let directory = TempDir::new().unwrap();
    let output = command(directory.path())
      .env("FAKE_MODE", mode)
      .args(["--prompt", "horse"])
      .output()
      .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    let calls = fs::read_to_string(directory.path().join("calls.log")).unwrap();
    assert!(!calls.contains("run --width"));
    assert!(!calls.contains("pull"));
    assert!(!directory.path().join("output-0001.png").exists());
  }
}

#[test]
fn cli_failure_missing_corrupt_multiple_and_wrong_size_images_never_succeed() {
  for mode in ["failure", "noimage", "corrupt", "multiple", "wrongsize"] {
    let directory = TempDir::new().unwrap();
    let mut command = command(directory.path());
    if mode == "wrongsize" {
      image::RgbImage::new(32, 32)
        .save(directory.path().join("fixture.png"))
        .unwrap();
    }
    let output = command
      .env("FAKE_MODE", mode)
      .args(["--prompt", "horse"])
      .output()
      .unwrap();
    assert!(!output.status.success(), "accepted {mode}");
    assert!(!directory.path().join("output-0001.png").exists());
    assert!(
      !directory
        .path()
        .join("output-0001.artifacts/result.json")
        .exists()
    );
    assert!(
      directory
        .path()
        .join("output-0001.artifacts/image.stdout.log")
        .exists()
    );
  }
}

#[test]
fn publishing_never_overwrites_file_created_during_inference() {
  let directory = TempDir::new().unwrap();
  let output_path = directory.path().join("output-0001.png");
  let output = command(directory.path())
    .env("FAKE_MODE", "race")
    .env("FAKE_RACE_OUTPUT", &output_path)
    .args(["--prompt", "horse"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert_eq!(
    fs::read_to_string(output_path).unwrap(),
    "created by someone else"
  );
  assert!(
    !directory
      .path()
      .join("output-0001.artifacts/result.json")
      .exists()
  );
}

#[test]
fn optional_text_optimization_uses_cli_and_saves_json() {
  let directory = TempDir::new().unwrap();
  let output = command(directory.path())
    .args(["--prompt", "马", "--model", "text:8b"])
    .output()
    .unwrap();
  success(&output);
  assert_eq!(
    saved(directory.path(), "response.json")["prompt"],
    "A running horse"
  );
  assert_eq!(
    saved(directory.path(), "generation.json")["prompt"],
    "A running horse"
  );
  let calls = fs::read_to_string(directory.path().join("calls.log")).unwrap();
  assert!(calls.contains("run --format json --hidethinking --nowordwrap -- text:8b"));
}

#[test]
fn text_errors_preserve_response_and_do_not_start_image_generation() {
  for response in ["not JSON", r#"{"prompt":" "}"#, r#"{"other":"text"}"#] {
    let directory = TempDir::new().unwrap();
    let output = command(directory.path())
      .env("FAKE_RESPONSE", response)
      .args(["--prompt", "马", "--model", "text:8b"])
      .output()
      .unwrap();
    assert!(!output.status.success());
    assert!(
      directory
        .path()
        .join("output-0001.artifacts/response.json")
        .exists()
    );
    assert!(
      !directory
        .path()
        .join("output-0001.artifacts/generation.json")
        .exists()
    );
    assert!(
      !fs::read_to_string(directory.path().join("calls.log"))
        .unwrap()
        .contains("run --width")
    );
  }
}

#[test]
fn replay_and_skip_text_are_offline_in_dry_run() {
  for replay in [false, true] {
    let directory = TempDir::new().unwrap();
    fs::write(
      directory.path().join("response.json"),
      r#"{"prompt":"Saved horse"}"#,
    )
    .unwrap();
    let mut command = command(directory.path());
    command.env("OLLAMA_MODEL", "unavailable:8b").args([
      "--prompt",
      "Original horse",
      "--dry-run",
      "--ollama",
      "/not/installed",
    ]);
    if replay {
      command.args(["--response", "response.json"]);
    } else {
      command.arg("--skip-ollama");
    }
    success(&command.output().unwrap());
    assert_eq!(
      saved(directory.path(), "generation.json")["prompt"],
      if replay {
        "Saved horse"
      } else {
        "Original horse"
      }
    );
    assert!(!directory.path().join("calls.log").exists());
  }
}

#[test]
fn reads_multiple_prompts_without_reusing_output_names() {
  let directory = TempDir::new().unwrap();
  let mut child = command(directory.path())
    .arg("--dry-run")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all("马\n\n草原\n".as_bytes())
    .unwrap();
  success(&child.wait_with_output().unwrap());
  assert!(
    directory
      .path()
      .join("output-0001.artifacts/generation.json")
      .exists()
  );
  assert!(
    directory
      .path()
      .join("output-0002.artifacts/generation.json")
      .exists()
  );
  assert!(!directory.path().join("output-0003.artifacts").exists());
}

#[test]
fn list_models_uses_cli_without_creating_generation_artifacts() {
  let directory = TempDir::new().unwrap();
  let output = command(directory.path())
    .arg("--list-models")
    .output()
    .unwrap();
  success(&output);
  assert!(String::from_utf8_lossy(&output.stdout).contains("x/z-image-turbo"));
  assert!(!directory.path().join("output-0001.artifacts").exists());
}

#[test]
fn timeout_stops_cli_and_does_not_publish_image() {
  let directory = TempDir::new().unwrap();
  let output = command(directory.path())
    .env("FAKE_MODE", "timeout")
    .args(["--prompt", "horse", "--timeout", "1"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("超时"));
  assert!(!directory.path().join("output-0001.png").exists());
}

#[test]
fn relative_cli_path_still_works_from_image_temporary_directory() {
  let directory = TempDir::new().unwrap();
  let output = command(directory.path())
    .args(["--prompt", "horse", "--ollama", "./fake-ollama"])
    .output()
    .unwrap();
  success(&output);
  assert!(directory.path().join("output-0001.png").exists());
}
