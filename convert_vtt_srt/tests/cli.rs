use std::{
  fs,
  path::PathBuf,
  process::{Command, Output},
  sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);

impl Fixture {
  fn new() -> Self {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
      "convert-vtt-srt-test-{}-{}",
      std::process::id(),
      NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    Self(path)
  }

  fn run(&self, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_convert_vtt_srt"))
      .current_dir(&self.0)
      .args(args)
      .output()
      .unwrap()
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

const VTT: &str = "\u{feff}WEBVTT\r\n\r\nNOTE ignore this\r\nmetadata\r\n\r\ncue-id\r\n00:01.250 \
                   --> 00:03.500 align:start\r\n你好 world\r\nsecond line\r\n\r\n00:00:04.000 --> \
                   00:00:05.000\r\nEnd\r\n";

#[test]
fn recursive_conversion_preview_skip_and_overwrite() {
  let fixture = Fixture::new();
  let nested = fixture.0.join(".hidden/中文 目录");
  fs::create_dir_all(&nested).unwrap();
  let input = nested.join("episode.VTT");
  let output = input.with_extension("srt");
  fs::write(&input, VTT).unwrap();
  let preview = fixture.run(&["--dry-run", "--ffmpeg", "/nonexistent/ffmpeg"]);
  assert!(preview.status.success());
  assert!(!output.exists());

  let converted = fixture.run(&[".", ".hidden"]);
  assert!(converted.status.success(), "{converted:?}");
  assert!(String::from_utf8_lossy(&converted.stdout).contains("找到 1 个 VTT"));
  let text = fs::read_to_string(&output).unwrap().replace("\r\n", "\n");
  assert_eq!(
    text,
    "1\n00:00:01,250 --> 00:00:03,500\n你好 world\nsecond line\n\n2\n00:00:04,000 --> \
     00:00:05,000\nEnd\n\n"
  );
  assert_eq!(fs::read_to_string(&input).unwrap(), VTT);

  fs::write(&output, "existing subtitle").unwrap();
  assert!(fixture.run(&[]).status.success());
  assert_eq!(fs::read_to_string(&output).unwrap(), "existing subtitle");
  let overwritten = fixture.run(&["--overwrite"]);
  assert!(overwritten.status.success(), "{overwritten:?}");
  assert!(fs::read_to_string(&output).unwrap().contains("你好 world"));
}

#[test]
fn failed_conversion_preserves_output_and_continues() {
  let fixture = Fixture::new();
  fs::write(fixture.0.join("a.vtt"), "this is not WebVTT").unwrap();
  fs::write(fixture.0.join("a.srt"), "keep me").unwrap();
  fs::write(fixture.0.join("b.vtt"), VTT).unwrap();
  let result = fixture.run(&["--overwrite"]);
  assert_eq!(result.status.code(), Some(1), "{result:?}");
  assert_eq!(
    fs::read_to_string(fixture.0.join("a.srt")).unwrap(),
    "keep me"
  );
  assert!(fixture.0.join("b.srt").exists());
  assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 4);
}

#[test]
fn arguments_missing_inputs_and_missing_ffmpeg() {
  let fixture = Fixture::new();
  assert!(fixture.run(&[]).status.success());
  assert!(fixture.run(&["--help"]).status.success());
  assert_eq!(fixture.run(&["--unknown"]).status.code(), Some(2));
  assert_eq!(fixture.run(&["--ffmpeg"]).status.code(), Some(2));
  assert_eq!(fixture.run(&["missing"]).status.code(), Some(1));
  fs::write(fixture.0.join("-episode.vtt"), VTT).unwrap();
  assert!(fixture.run(&["-n", "--", "-episode.vtt"]).status.success());
  let result = fixture.run(&["--ffmpeg", "/nonexistent/ffmpeg"]);
  assert_eq!(result.status.code(), Some(1));
  assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn does_not_follow_symlinks_or_replace_dangling_output_by_default() {
  use std::os::unix::fs::symlink;
  let fixture = Fixture::new();
  fs::write(fixture.0.join("input.vtt"), VTT).unwrap();
  symlink(".", fixture.0.join("loop")).unwrap();
  symlink("input.vtt", fixture.0.join("alias.vtt")).unwrap();
  symlink("missing.srt", fixture.0.join("input.srt")).unwrap();
  assert!(fixture.run(&[]).status.success());
  assert!(!fixture.0.join("alias.srt").exists());
  assert!(!fixture.0.join("missing.srt").exists());
  assert_eq!(
    fs::read_link(fixture.0.join("input.srt")).unwrap(),
    PathBuf::from("missing.srt")
  );
}
