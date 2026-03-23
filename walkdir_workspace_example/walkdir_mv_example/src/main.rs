use std::{
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

  println!(
    "Done. Updated {changed} directories (checked only direct children of {})",
    root.display()
  );
  Ok(())
}

fn flatten_if_single_child_dir(dir: &Path) -> io::Result<bool> {
  if !dir.is_dir() {
    return Ok(false);
  }

  let entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, io::Error>>()?;
  let child_dirs: Vec<PathBuf> = entries
    .into_iter()
    .filter_map(|entry| match entry.file_type() {
      Ok(ft) if ft.is_dir() => Some(entry.path()),
      _ => None,
    })
    .collect();

  if child_dirs.len() != 1 {
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
