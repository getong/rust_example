use std::{
  ffi::{OsStr, OsString},
  fs, io,
  path::{Path, PathBuf},
};

fn main() {
  let target = match resolve_target_dir() {
    Ok(dir) => dir,
    Err(err) => {
      eprintln!("{err}");
      std::process::exit(2);
    }
  };
  let root = &target;

  if let Err(err) = process_all_dirs(root) {
    eprintln!("Processing failed: {err}");
    std::process::exit(1);
  }
}

fn resolve_target_dir() -> Result<PathBuf, String> {
  parse_target_dir(
    std::env::args_os().skip(1),
    std::env::var_os("WALKDIR_DELETE_TARGET_DIR"),
  )
}

fn parse_target_dir(
  args: impl IntoIterator<Item = OsString>,
  env_dir: Option<OsString>,
) -> Result<PathBuf, String> {
  let mut args = args.into_iter();
  let mut target = None;
  while let Some(arg) = args.next() {
    let value = if arg == "--dir" {
      let value = args.next().ok_or("Missing value: use `--dir <path>`")?;
      if value.as_encoded_bytes().starts_with(b"--") {
        return Err("Missing directory value; use `--dir=./--name` for such paths".into());
      }
      value
    } else if let Some(value) = arg.to_str().and_then(|arg| arg.strip_prefix("--dir=")) {
      OsString::from(value)
    } else {
      return Err(format!("Unknown argument: {}", arg.to_string_lossy()));
    };
    if value.to_string_lossy().trim().is_empty() {
      return Err("Directory cannot be empty".into());
    }
    if target.replace(PathBuf::from(value)).is_some() {
      return Err("Specify --dir only once".into());
    }
  }
  if target.is_none()
    && let Some(value) = env_dir
  {
    if value.to_string_lossy().trim().is_empty() {
      return Err("WALKDIR_DELETE_TARGET_DIR cannot be empty".into());
    }
    target = Some(PathBuf::from(value));
  }
  target.ok_or_else(|| "Missing target: use `--dir <path>`".into())
}

fn process_all_dirs(root: &Path) -> io::Result<()> {
  // Only inspect direct children. Do not recursively clean unrelated files or directories.
  let entries = fs::read_dir(root)?.collect::<io::Result<Vec<_>>>()?;
  let mut child_dirs = Vec::new();
  for entry in entries {
    if entry.file_type()?.is_dir() && !is_hidden_name(&entry.file_name()) {
      child_dirs.push(entry.path());
    }
  }

  let mut changed = 0usize;
  for dir in child_dirs {
    if flatten_if_single_child_dir(&dir)? {
      changed += 1;
    }
  }
  println!(
    "Done. Updated {changed} directories (checked only direct children of {})",
    root.display()
  );
  Ok(())
}

// Fail closed on unsupported platforms/filesystems; never fall back to an overwriting rename.
#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
fn move_no_replace(src: &Path, dst: &Path) -> io::Result<()> {
  use rustix::fs::{CWD, RenameFlags, renameat_with};
  renameat_with(CWD, src, CWD, dst, RenameFlags::NOREPLACE)?;
  Ok(())
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
fn move_no_replace(_src: &Path, _dst: &Path) -> io::Result<()> {
  Err(io::Error::new(
    io::ErrorKind::Unsupported,
    "Non-overwriting moves are unsupported on this platform",
  ))
}

fn flatten_if_single_child_dir(dir: &Path) -> io::Result<bool> {
  if !fs::symlink_metadata(dir)?.file_type().is_dir() {
    return Ok(false);
  }

  let entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, io::Error>>()?;
  let mut child_dirs = Vec::new();
  let mut visible_non_dir_count = 0usize;

  for entry in entries {
    if is_hidden_name(entry.file_name().as_os_str()) {
      continue;
    }

    match entry.file_type()? {
      ft if ft.is_dir() => child_dirs.push(entry.path()),
      _ => visible_non_dir_count += 1,
    }
  }

  if child_dirs.len() != 1 || visible_non_dir_count != 0 {
    return Ok(false);
  }

  let child_dir = child_dirs[0].clone();
  let child_entries = fs::read_dir(&child_dir)?.collect::<Result<Vec<_>, io::Error>>()?;

  println!(
    "Matched directory: {} -> only child directory: {}",
    dir.display(),
    child_dir.display()
  );

  let mut touched = false;
  let mut moved_any_entry = false;
  for item in child_entries {
    let src = item.path();
    let dst = dir.join(item.file_name());

    match move_no_replace(&src, &dst) {
      Ok(()) => {}
      Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
        eprintln!(
          "Skipped (destination exists): {} -> {}",
          src.display(),
          dst.display()
        );
        continue;
      }
      Err(err) => {
        return Err(io::Error::new(
          err.kind(),
          format!("Cannot move {} -> {}: {err}", src.display(), dst.display()),
        ));
      }
    }
    touched = true;
    moved_any_entry = true;
    println!("Moved: {} -> {}", src.display(), dst.display());
  }

  if !moved_any_entry {
    println!("Skip deletion (no entries moved): {}", child_dir.display());
    return Ok(touched);
  }

  match fs::remove_dir(&child_dir) {
    Ok(()) => {
      touched = true;
      println!("Deleted empty directory: {}", child_dir.display());
    }
    Err(err) => {
      eprintln!(
        "Directory not deleted (possibly leftover due to name conflicts): {} ({err})",
        child_dir.display()
      );
    }
  }

  Ok(touched)
}

fn is_hidden_name(name: &OsStr) -> bool {
  name.to_string_lossy().starts_with('.')
}
