use std::{
  fs,
  path::{Path, PathBuf},
  process::{Command, ExitStatus},
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use tempfile::{Builder, TempPath};

#[derive(Parser, Debug)]
#[command(author, version, about = "Merge video and audio files with ffmpeg")]
struct Cli {
  /// First input file
  first: PathBuf,
  /// Second input file
  second: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
  let cli = Cli::parse();
  ensure!(cli.first != cli.second, "expected two different files");

  let (larger_file, smaller_file) = order_by_size(&cli.first, &cli.second)?;
  let (_temp_path, output_path) = tempfile_output_path(&larger_file)?;

  run_ffmpeg(&larger_file, &smaller_file, &output_path)?;
  cleanup_inputs(&larger_file, &smaller_file)?;
  fs::rename(&output_path, &larger_file).with_context(|| {
    format!(
      "failed to rename {} to {}",
      output_path.display(),
      larger_file.display()
    )
  })?;

  println!("{}", larger_file.display());
  Ok(())
}

fn order_by_size(first: &Path, second: &Path) -> Result<(PathBuf, PathBuf)> {
  let first_len = fs::metadata(first)
    .with_context(|| format!("failed to read metadata for {}", first.display()))?
    .len();
  let second_len = fs::metadata(second)
    .with_context(|| format!("failed to read metadata for {}", second.display()))?
    .len();

  if first_len >= second_len {
    Ok((first.to_path_buf(), second.to_path_buf()))
  } else {
    Ok((second.to_path_buf(), first.to_path_buf()))
  }
}

fn tempfile_output_path(larger_file: &Path) -> Result<(TempPath, PathBuf)> {
  let parent_dir = larger_file
    .parent()
    .map(Path::to_path_buf)
    .unwrap_or_else(|| PathBuf::from("."));
  let file_prefix = larger_file
    .file_stem()
    .and_then(|stem| stem.to_str())
    .filter(|stem| !stem.is_empty())
    .unwrap_or("merged");
  let temp_file = Builder::new()
    .prefix(&format!("{file_prefix}."))
    .suffix(".mp4")
    .tempfile_in(&parent_dir)
    .with_context(|| format!("failed to create temp file in {}", parent_dir.display()))?;
  let temp_path = temp_file.into_temp_path();
  let output_path = temp_path.to_path_buf();

  fs::remove_file(&output_path)
    .with_context(|| format!("failed to prepare temp output {}", output_path.display()))?;

  Ok((temp_path, output_path))
}

fn run_ffmpeg(larger_file: &Path, smaller_file: &Path, output_path: &Path) -> Result<()> {
  let status = Command::new("ffmpeg")
    .arg("-i")
    .arg(larger_file)
    .arg("-i")
    .arg(smaller_file)
    .arg("-c:v")
    .arg("copy")
    .arg("-c:a")
    .arg("aac")
    .arg("-shortest")
    .arg(output_path)
    .status()
    .with_context(|| "failed to run ffmpeg")?;

  ensure_success(status)
}

fn ensure_success(status: ExitStatus) -> Result<()> {
  if status.success() {
    Ok(())
  } else {
    bail!("ffmpeg exited with status {status}")
  }
}

fn cleanup_inputs(larger_file: &Path, smaller_file: &Path) -> Result<()> {
  fs::remove_file(larger_file)
    .with_context(|| format!("failed to remove {}", larger_file.display()))?;
  fs::remove_file(smaller_file)
    .with_context(|| format!("failed to remove {}", smaller_file.display()))?;
  Ok(())
}
