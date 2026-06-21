use std::{fs, io, path::Path, process::Command, sync::OnceLock};

use regex::Regex;

const UNRAR_DIRECTORY: &str = "UNRAR_DIRECTORY";
const MULTIPART_ARCHIVE_REGEX: &str = r"(?i)^(?P<prefix>.*part)(?P<number>\d+)(?P<suffix>.*\.rar)$";

#[derive(Debug)]
enum ArchiveKind {
  FirstMultipart(Regex),
  LaterMultipart,
  SingleArchive,
}

#[tokio::main]
async fn main() {
  if let Err(error) = run() {
    eprintln!("error: {error}");
  }
}

fn run() -> io::Result<()> {
  let Ok(directory) = dotenv::var(UNRAR_DIRECTORY) else {
    return Ok(());
  };

  let directory = Path::new(&directory);
  for entry in fs::read_dir(directory)? {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) => {
        eprintln!("error reading directory entry: {error}");
        continue;
      }
    };

    let path = entry.path();
    if path.extension().and_then(|ext| ext.to_str()) != Some("rar") {
      continue;
    }

    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
      continue;
    };

    match archive_kind(file_name) {
      ArchiveKind::FirstMultipart(delete_pattern) => {
        if path.exists() && call_unrar_command(directory, file_name) {
          delete_all_files(directory, &delete_pattern)?;
        }
      }
      ArchiveKind::LaterMultipart => {}
      ArchiveKind::SingleArchive => {
        if path.exists() && call_unrar_command(directory, file_name) {
          call_trash_command(directory, file_name);
        }
      }
    }
  }

  Ok(())
}

fn archive_kind(file_name: &str) -> ArchiveKind {
  let Some(captures) = multipart_archive_regex().captures(file_name) else {
    return ArchiveKind::SingleArchive;
  };

  let Some(number) = captures
    .name("number")
    .and_then(|number| number.as_str().parse::<u64>().ok())
  else {
    return ArchiveKind::LaterMultipart;
  };

  if number != 1 {
    return ArchiveKind::LaterMultipart;
  }

  let prefix = regex::escape(captures.name("prefix").map_or("", |prefix| prefix.as_str()));
  let suffix = regex::escape(captures.name("suffix").map_or("", |suffix| suffix.as_str()));
  let delete_pattern = Regex::new(&format!(r"(?i)^{}\d+{}$", prefix, suffix))
    .expect("multipart delete regex should be valid");

  ArchiveKind::FirstMultipart(delete_pattern)
}

fn multipart_archive_regex() -> &'static Regex {
  static REGEX: OnceLock<Regex> = OnceLock::new();
  REGEX.get_or_init(|| {
    Regex::new(MULTIPART_ARCHIVE_REGEX).expect("multipart archive regex should be valid")
  })
}

fn call_unrar_command(directory: &Path, file_name: &str) -> bool {
  match Command::new("unrar")
    .current_dir(directory)
    .arg("x")
    .arg(file_name)
    .status()
  {
    Ok(status) => status.success(),
    Err(error) => {
      eprintln!("failed to start unrar for {file_name:?}: {error}");
      false
    }
  }
}

fn call_trash_command(directory: &Path, file_name: &str) -> bool {
  println!("delete {:?}", file_name);
  match Command::new("trash-put")
    .current_dir(directory)
    .arg(file_name)
    .status()
  {
    Ok(status) if status.success() => {
      println!("rm {:?}", file_name);
      true
    }
    Ok(status) => {
      eprintln!("trash-put failed for {file_name:?} with status {status}");
      false
    }
    Err(error) => {
      eprintln!("failed to start trash-put for {file_name:?}: {error}");
      false
    }
  }
}

fn delete_all_files(directory: &Path, delete_pattern: &Regex) -> io::Result<()> {
  for entry in fs::read_dir(directory)? {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) => {
        eprintln!("error reading directory entry: {error}");
        continue;
      }
    };

    let path = entry.path();
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
      continue;
    };

    if path.exists() && delete_pattern.is_match(file_name) {
      call_trash_command(directory, file_name);
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn archive_kind_detects_first_multipart_archive() {
    let ArchiveKind::FirstMultipart(delete_pattern) =
      archive_kind("abc_2026-3.part01_Downloadly.ir.rar")
    else {
      panic!("part01 should be detected as the first multipart archive");
    };

    assert!(delete_pattern.is_match("abc_2026-3.part01_Downloadly.ir.rar"));
    assert!(delete_pattern.is_match("abc_2026-3.part10_Downloadly.ir.rar"));
    assert!(!delete_pattern.is_match("abc_2026-4.part10_Downloadly.ir.rar"));
  }

  #[test]
  fn archive_kind_skips_later_multipart_archives() {
    assert!(matches!(
      archive_kind("abc_2026-3.part10_Downloadly.ir.rar"),
      ArchiveKind::LaterMultipart
    ));
    assert!(matches!(
      archive_kind("abc_2026-3.part11_Downloadly.ir.rar"),
      ArchiveKind::LaterMultipart
    ));
  }

  #[test]
  fn archive_kind_keeps_single_archive_files() {
    assert!(matches!(
      archive_kind("abc_2026-3.rar"),
      ArchiveKind::SingleArchive
    ));
    assert!(matches!(
      archive_kind("abc_2026-3.partial.rar"),
      ArchiveKind::SingleArchive
    ));
  }
}
