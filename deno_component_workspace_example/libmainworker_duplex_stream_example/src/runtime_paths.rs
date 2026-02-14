use std::path::PathBuf;

use deno_core::{error::AnyError, url::Url};

pub(crate) fn resolve_target_specifier(arg: &str) -> Result<String, AnyError> {
  let is_url_like = arg.starts_with("file://")
    || arg.starts_with("http://")
    || arg.starts_with("https://")
    || arg.starts_with("jsr:")
    || arg.starts_with("npm:");

  if is_url_like {
    return Ok(arg.to_string());
  }

  let path = PathBuf::from(arg);
  let abs_path = if path.is_absolute() {
    path
  } else {
    let cwd = std::env::current_dir()?;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
      .parent()
      .map(PathBuf::from)
      .unwrap_or_else(|| manifest_dir.clone());

    let mut candidates = Vec::with_capacity(5);
    candidates.push(cwd.join(&path));
    candidates.push(manifest_dir.join(&path));
    candidates.push(workspace_root.join(&path));
    candidates.push(cwd.join("embed_deno").join(&path));
    candidates.push(workspace_root.join("embed_deno").join(&path));

    if let Some(found) = candidates.iter().find(|candidate| candidate.is_file()) {
      found.clone()
    } else {
      let tried = candidates
        .into_iter()
        .map(|candidate| candidate.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
      return Err(AnyError::msg(format!(
        "target script not found for `{arg}`; looked in: {tried}"
      )));
    }
  };

  Url::from_file_path(&abs_path)
    .map(|url| url.to_string())
    .map_err(|_| {
      AnyError::msg(format!(
        "failed to convert path to file url: {}",
        abs_path.display()
      ))
    })
}

pub(crate) fn bootstrap_script_path() -> Result<PathBuf, AnyError> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("src")
    .join("duplex_bootstrap.ts");
  if !path.exists() {
    return Err(AnyError::msg(format!(
      "duplex bootstrap script not found: {}",
      path.display()
    )));
  }
  Ok(path)
}
