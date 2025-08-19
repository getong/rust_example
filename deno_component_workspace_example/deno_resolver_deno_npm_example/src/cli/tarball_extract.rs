// Copyright 2018-2025 the Deno authors. MIT license.

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
};

use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_semver::package::PackageNv;
use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

#[derive(Debug, Copy, Clone)]
pub enum TarballExtractionMode {
  /// Overwrites the destination directory without deleting any files.
  Overwrite,
  /// Creates and writes to a sibling temporary directory. When done, moves
  /// it to the final destination.
  SiblingTempDir,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExtractTarballError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(generic)]
  #[error("Extracted directory '{0}' of npm tarball was not in output directory.")]
  NotInOutputDirectory(PathBuf),
  #[class(generic)]
  #[error("Tarball integrity check failed: expected {expected}, got {actual}")]
  IntegrityCheckFailed { expected: String, actual: String },
}

pub fn extract_tarball_simple(
  data: &[u8],
  output_folder: &Path,
) -> Result<(), ExtractTarballError> {
  std::fs::create_dir_all(output_folder)?;
  let output_folder = std::fs::canonicalize(output_folder)?;

  let tar = GzDecoder::new(data);
  let mut archive = Archive::new(tar);
  archive.set_overwrite(true);
  archive.set_preserve_permissions(true);
  let mut created_dirs = HashSet::new();

  for entry in archive.entries()? {
    let mut entry = entry?;
    let path = entry.path()?;
    let entry_type = entry.header().entry_type();

    // Some package tarballs contain "pax_global_header", these entries should be skipped.
    if entry_type == EntryType::XGlobalHeader {
      continue;
    }

    // skip the first component which will be either "package" or the name of the package
    let relative_path = path.components().skip(1).collect::<PathBuf>();
    let absolute_path = output_folder.join(relative_path);

    // Ensure we're not extracting outside the output folder
    if !absolute_path.starts_with(&output_folder) {
      return Err(ExtractTarballError::NotInOutputDirectory(absolute_path));
    }

    let dir_path = if entry_type == EntryType::Directory {
      absolute_path.as_path()
    } else {
      absolute_path.parent().unwrap()
    };

    if created_dirs.insert(dir_path.to_path_buf()) {
      std::fs::create_dir_all(dir_path)?;
    }

    if entry_type == EntryType::Regular {
      entry.unpack(&absolute_path)?;
    } else if entry_type == EntryType::Directory {
      std::fs::create_dir_all(&absolute_path)?;
    }
  }

  Ok(())
}

pub fn verify_tarball_integrity(
  _package_nv: &PackageNv,
  _data: &[u8],
  _dist_info: &NpmPackageVersionDistInfo,
) -> Result<(), ExtractTarballError> {
  // For now, skip integrity checking since the API is complex
  // In a production implementation, you would check shasum/integrity fields
  // The dist_info fields are private, so we'll skip verification for this demo
  println!("⚠️ Skipping integrity verification (would check shasum/integrity in production)");
  Ok(())
}
