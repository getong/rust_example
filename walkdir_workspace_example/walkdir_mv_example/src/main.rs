use std::{
  ffi::OsStr,
  fs, io,
  path::{Path, PathBuf},
};

use walkdir::WalkDir;

fn main() {
  let target = match resolve_target_dir() {
    Ok(dir) => dir,
    Err(err) => {
      eprintln!("{err}");
      std::process::exit(2);
    }
  };
  let root = Path::new(&target);

  if let Err(err) = process_all_dirs(root) {
    eprintln!("Processing failed: {err}");
    std::process::exit(1);
  }
}

fn resolve_target_dir() -> Result<String, String> {
  let mut args = std::env::args().skip(1);
  while let Some(arg) = args.next() {
    if let Some(value) = arg.strip_prefix("--dir=") {
      if value.trim().is_empty() {
        return Err("Invalid `--dir=` value: directory cannot be empty".to_string());
      }
      return Ok(value.to_string());
    }

    if arg == "--dir" {
      let value = args
        .next()
        .ok_or_else(|| "Missing value: use `--dir <path>`".to_string())?;
      if value.trim().is_empty() {
        return Err("Invalid `--dir` value: directory cannot be empty".to_string());
      }
      return Ok(value);
    }
  }

  if let Ok(dir) = std::env::var("WALKDIR_DELETE_TARGET_DIR") {
    if !dir.trim().is_empty() {
      return Ok(dir);
    }
  }

  Ok(".".to_string())
}

fn process_all_dirs(root: &Path) -> io::Result<()> {
  let deleted_ds_store = delete_ds_store_files(root)?;

  let child_dirs: Vec<PathBuf> = WalkDir::new(root)
    .min_depth(1)
    .max_depth(1)
    .follow_links(false)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|e| e.file_type().is_dir())
    .map(|e| e.path().to_path_buf())
    .collect();

  let mut changed = 0usize;
  for dir in child_dirs {
    if flatten_if_single_child_dir(&dir)? {
      changed += 1;
    }
  }

  let deleted_empty_dirs = delete_empty_dirs(root)?;

  println!(
    "Done. Removed {deleted_ds_store} .DS_Store files, updated {changed} directories (checked \
     only direct children of {}), deleted {deleted_empty_dirs} empty directories",
    root.display(),
  );
  Ok(())
}

fn delete_ds_store_files(root: &Path) -> io::Result<usize> {
  let ds_store_paths: Vec<PathBuf> = WalkDir::new(root)
    .follow_links(false)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file() && entry.file_name() == OsStr::new(".DS_Store"))
    .map(|entry| entry.into_path())
    .collect();

  let mut deleted = 0usize;
  for path in ds_store_paths {
    fs::remove_file(&path)?;
    deleted += 1;
    println!("Deleted .DS_Store: {}", path.display());
  }

  Ok(deleted)
}

fn delete_empty_dirs(root: &Path) -> io::Result<usize> {
  let dirs: Vec<PathBuf> = WalkDir::new(root)
    .min_depth(1)
    .contents_first(true)
    .follow_links(false)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_dir())
    .map(|entry| entry.into_path())
    .collect();

  let mut deleted = 0usize;
  for dir in dirs {
    if fs::read_dir(&dir)?.next().is_none() {
      fs::remove_dir(&dir)?;
      deleted += 1;
      println!("Deleted empty directory: {}", dir.display());
    }
  }

  Ok(deleted)
}

fn flatten_if_single_child_dir(dir: &Path) -> io::Result<bool> {
  if !dir.is_dir() {
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

    if dst.exists() {
      eprintln!(
        "Skipped (destination exists): {} -> {}",
        src.display(),
        dst.display()
      );
      continue;
    }

    fs::rename(&src, &dst)?;
    touched = true;
    moved_any_entry = true;
    println!("Moved: {} -> {}", src.display(), dst.display());
  }

  if !moved_any_entry {
    println!(
      "Skip deletion (only child directory is empty): {}",
      child_dir.display()
    );
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
