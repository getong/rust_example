//! 查找并（可选）删除 rsync / cp 到非 HFS 文件系统时产生的隐藏文件。
//!
//! 默认只做 dry-run（类似 `find`，只打印），加 `--delete` 才真正删除。
//!
//! 匹配规则（全部满足才算）：
//!   AppleDouble 文件 `._xxx`：
//!     1. 文件名以 `._` 开头，且后面还有内容
//!     2. 是普通文件（不是目录、不是符号链接）
//!     3. 文件头 magic = 0x00051607，version = 0x00020000（AppleDouble v2）
//!     4. entry 数量与文件大小一致
//!   原始文件 `xxx` 已不存在的孤儿 `._xxx` 也会删除（文件头已足以证明是 AppleDouble），
//!   如需保留孤儿文件加 `--keep-orphans`
//!   rsync 中断残留的临时文件 `.xxx.AbC123`（需 `--include-rsync-temp`）：
//!     1. 文件名形如 `.` + 原名 + `.` + 6 位字母数字
//!     2. 是普通文件
//!     3. 同目录下存在原始文件 `xxx`
//!
//! 用法：
//!   cargo run -p delete_rsync_hidden_files -- <DIR> [--delete] [--keep-orphans]
//! [--include-rsync-temp]

use std::{
  ffi::OsStr,
  fs::{self, File},
  io::{self, Read},
  os::unix::ffi::OsStrExt,
  path::{Path, PathBuf},
  process::ExitCode,
};

const APPLE_DOUBLE_MAGIC: [u8; 4] = [0x00, 0x05, 0x16, 0x07];
const APPLE_DOUBLE_VERSION: [u8; 4] = [0x00, 0x02, 0x00, 0x00];
/// magic(4) + version(4) + filler(16) + entry 数量(2)
const APPLE_DOUBLE_HEADER_LEN: usize = 26;
/// 每个 entry 描述符：id(4) + offset(4) + length(4)
const APPLE_DOUBLE_ENTRY_LEN: u64 = 12;

#[derive(Debug, Default)]
struct Options {
  root: Option<PathBuf>,
  delete: bool,
  keep_orphans: bool,
  include_rsync_temp: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
  AppleDouble,
  AppleDoubleOrphan,
  RsyncTemp,
}

impl Kind {
  fn label(self) -> &'static str {
    match self {
      Kind::AppleDouble => "appledouble",
      Kind::AppleDoubleOrphan => "appledouble-orphan",
      Kind::RsyncTemp => "rsync-temp",
    }
  }
}

#[derive(Debug, Default)]
struct Stats {
  scanned_hidden: usize,
  apple_double: usize,
  orphans_skipped: usize,
  orphans_matched: usize,
  rsync_temp: usize,
  rejected: usize,
  deleted: usize,
  failed: usize,
}

fn parse_args() -> Result<Options, String> {
  let mut opts = Options::default();
  for arg in std::env::args_os().skip(1) {
    match arg.to_str() {
      Some("--delete") => opts.delete = true,
      Some("--keep-orphans") => opts.keep_orphans = true,
      Some("--include-rsync-temp") => opts.include_rsync_temp = true,
      Some(s) if s.starts_with("--") => return Err(format!("unknown option: {s}")),
      _ if opts.root.is_none() => opts.root = Some(PathBuf::from(arg)),
      _ => return Err("only one directory is allowed".into()),
    }
  }
  if opts.root.is_none() {
    return Err(
      "usage: delete_rsync_hidden_files <DIR> [--delete] [--keep-orphans] [--include-rsync-temp]"
        .into(),
    );
  }
  Ok(opts)
}

/// 检查文件头是否是合法的 AppleDouble v2 结构。
fn is_apple_double(path: &Path, len: u64) -> io::Result<bool> {
  if len < APPLE_DOUBLE_HEADER_LEN as u64 {
    return Ok(false);
  }
  let mut header = [0u8; APPLE_DOUBLE_HEADER_LEN];
  File::open(path)?.read_exact(&mut header)?;
  if header[0 .. 4] != APPLE_DOUBLE_MAGIC || header[4 .. 8] != APPLE_DOUBLE_VERSION {
    return Ok(false);
  }
  let entries = u16::from_be_bytes([header[24], header[25]]) as u64;
  Ok(APPLE_DOUBLE_HEADER_LEN as u64 + entries * APPLE_DOUBLE_ENTRY_LEN <= len)
}

/// rsync 临时文件名：`.` + 原名 + `.` + 6 位 [A-Za-z0-9]，返回原名。
fn rsync_temp_original(name: &[u8]) -> Option<&[u8]> {
  let rest = name.strip_prefix(b".")?;
  if rest.len() < 8 || rest.starts_with(b".") {
    return None;
  }
  let (orig, suffix) = rest.split_at(rest.len() - 7);
  let (dot, rand) = suffix.split_first()?;
  (*dot == b'.' && !orig.is_empty() && rand.iter().all(u8::is_ascii_alphanumeric)).then_some(orig)
}

fn classify(path: &Path, name: &[u8], len: u64, opts: &Options, stats: &mut Stats) -> Option<Kind> {
  let dir = path.parent()?;

  if let Some(orig) = name.strip_prefix(b"._").filter(|o| !o.is_empty()) {
    match is_apple_double(path, len) {
      Ok(true) => {}
      Ok(false) => {
        stats.rejected += 1;
        eprintln!("skip (not AppleDouble): {}", path.display());
        return None;
      }
      Err(e) => {
        stats.rejected += 1;
        eprintln!("skip (read error {e}): {}", path.display());
        return None;
      }
    }
    // 用 symlink_metadata 判断存在性，原始文件是符号链接也算存在
    if fs::symlink_metadata(dir.join(OsStr::from_bytes(orig))).is_ok() {
      stats.apple_double += 1;
      return Some(Kind::AppleDouble);
    }
    if opts.keep_orphans {
      stats.orphans_skipped += 1;
      eprintln!("skip (orphan, original missing): {}", path.display());
      return None;
    }
    stats.orphans_matched += 1;
    return Some(Kind::AppleDoubleOrphan);
  }

  if opts.include_rsync_temp {
    if let Some(orig) = rsync_temp_original(name) {
      if fs::symlink_metadata(dir.join(OsStr::from_bytes(orig))).is_ok() {
        stats.rsync_temp += 1;
        return Some(Kind::RsyncTemp);
      }
    }
  }
  None
}

/// 递归遍历，不跟随符号链接（与 `find` 默认行为一致）。
fn walk(dir: &Path, opts: &Options, stats: &mut Stats, out: &mut Vec<(PathBuf, Kind)>) {
  let entries = match fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(e) => {
      eprintln!("cannot read dir {}: {e}", dir.display());
      return;
    }
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let Ok(file_type) = entry.file_type() else {
      continue;
    };
    if file_type.is_dir() {
      walk(&path, opts, stats, out);
      continue;
    }
    let name = entry.file_name();
    let name = name.as_bytes();
    if !name.starts_with(b".") || !file_type.is_file() {
      continue;
    }
    stats.scanned_hidden += 1;
    let Ok(meta) = entry.metadata() else { continue };
    if let Some(kind) = classify(&path, name, meta.len(), opts, stats) {
      out.push((path, kind));
    }
  }
}

fn main() -> ExitCode {
  let opts = match parse_args() {
    Ok(opts) => opts,
    Err(msg) => {
      eprintln!("{msg}");
      return ExitCode::from(2);
    }
  };
  let root = opts.root.as_deref().expect("checked in parse_args");
  if !root.is_dir() {
    eprintln!("not a directory: {}", root.display());
    return ExitCode::from(2);
  }

  let mut stats = Stats::default();
  let mut matched = Vec::new();
  walk(root, &opts, &mut stats, &mut matched);
  matched.sort_by(|a, b| a.0.cmp(&b.0));

  for (path, kind) in &matched {
    if opts.delete {
      match fs::remove_file(path) {
        Ok(()) => {
          stats.deleted += 1;
          println!("deleted [{}] {}", kind.label(), path.display());
        }
        Err(e) => {
          stats.failed += 1;
          eprintln!("delete failed [{}] {}: {e}", kind.label(), path.display());
        }
      }
    } else {
      println!("[{}] {}", kind.label(), path.display());
    }
  }

  eprintln!();
  eprintln!("hidden files scanned : {}", stats.scanned_hidden);
  eprintln!("appledouble matched  : {}", stats.apple_double);
  eprintln!("orphans matched      : {}", stats.orphans_matched);
  eprintln!("orphans skipped      : {}", stats.orphans_skipped);
  eprintln!("rsync temp matched   : {}", stats.rsync_temp);
  eprintln!("`._` rejected        : {}", stats.rejected);
  if opts.delete {
    eprintln!("deleted              : {}", stats.deleted);
    eprintln!("failed               : {}", stats.failed);
  } else {
    eprintln!("(dry-run, add --delete to remove {} files)", matched.len());
  }

  if stats.failed > 0 {
    ExitCode::FAILURE
  } else {
    ExitCode::SUCCESS
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rsync_temp_name() {
    assert_eq!(rsync_temp_original(b".a.mp4.sUyrwT"), Some(&b"a.mp4"[..]));
    assert_eq!(rsync_temp_original(b".DS_Store"), None);
    assert_eq!(rsync_temp_original(b".abc.js"), None);
    assert_eq!(rsync_temp_original(b".a.mp4.sUy-wT"), None);
  }
}
