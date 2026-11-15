use std::{
  cmp::Ordering,
  fs,
  path::{Path, PathBuf},
};

pub fn from_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
  if let [current] = paths.as_slice() {
    from_directory(current)
  } else {
    paths
  }
}

fn from_directory(current: &Path) -> Vec<PathBuf> {
  let mut paths = current
    .parent()
    .and_then(|parent| fs::read_dir(parent).ok())
    .into_iter()
    .flatten()
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| crate::media::supported(path) && path.is_file())
    .collect::<Vec<_>>();
  if !paths.iter().any(|path| path == current) {
    paths.push(current.to_path_buf());
  }
  paths.sort_by(|left, right| {
    let left_name = left.file_name().unwrap_or_default();
    let right_name = right.file_name().unwrap_or_default();
    natural_cmp(
      &left_name.to_string_lossy().to_lowercase(),
      &right_name.to_string_lossy().to_lowercase(),
    )
    .then_with(|| left_name.cmp(right_name))
  });
  paths
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
  let mut left = left.as_bytes();
  let mut right = right.as_bytes();
  while let (Some(left_byte), Some(right_byte)) = (left.first(), right.first()) {
    if left_byte.is_ascii_digit() && right_byte.is_ascii_digit() {
      let left_end = left
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(left.len());
      let right_end = right
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(right.len());
      let left_number = &left[.. left_end];
      let right_number = &right[.. right_end];
      let left_number = &left_number[left_number
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(left_end) ..];
      let right_number = &right_number[right_number
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(right_end) ..];
      let ordering = left_number
        .len()
        .cmp(&right_number.len())
        .then_with(|| left_number.cmp(right_number));
      if ordering != Ordering::Equal {
        return ordering;
      }
      left = &left[left_end ..];
      right = &right[right_end ..];
    } else {
      let ordering = left_byte.cmp(right_byte);
      if ordering != Ordering::Equal {
        return ordering;
      }
      left = &left[1 ..];
      right = &right[1 ..];
    }
  }
  left.len().cmp(&right.len())
}
