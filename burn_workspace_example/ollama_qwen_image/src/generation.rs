use std::{
  fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{cli::Args, ollama};

#[derive(Serialize)]
pub struct Request {
  pub backend: &'static str,
  pub model: String,
  pub prompt: String,
  pub width: u32,
  pub height: u32,
  pub steps: u32,
  pub seed: i64,
  pub output: PathBuf,
}

impl Request {
  pub fn new(args: &Args, prompt: String, output: PathBuf) -> Self {
    Self {
      backend: "ollama-cli",
      model: args.image_model.clone(),
      prompt,
      width: args.width,
      height: args.height,
      steps: args.steps,
      seed: args.seed,
      output,
    }
  }
}

pub fn preflight(args: &Args, artifacts: &Path) -> Result<()> {
  let mut help = ollama::command(args);
  help.args(["run", "--help"]);
  let bytes = ollama::execute(&mut help, artifacts, "capabilities", args.timeout)?;
  let help = String::from_utf8_lossy(&bytes);
  ensure!(
    ["--width", "--height", "--steps", "--seed"]
      .iter()
      .all(|flag| help.contains(flag)),
    "当前 Ollama CLI 不支持图像生成；请执行 ./download.sh 准备兼容运行时，\
     或用 --ollama 指定支持图像生成的实验版本；不会退回 Python"
  );
  let mut show = ollama::command(args);
  show.args(["show", "--", &args.image_model]);
  ollama::execute(&mut show, artifacts, "image-model", args.timeout)
    .context("图像模型不可用；请先执行 ./download.sh（run.sh 不会隐式下载模型）")?;
  Ok(())
}

pub fn execute(args: &Args, request: &Request, artifacts: &Path) -> Result<()> {
  let workspace = tempfile::Builder::new()
    .prefix("ollama-image-")
    .tempdir_in(artifacts)?;
  let mut command = ollama::command(args);
  command.current_dir(workspace.path()).args([
    "run",
    "--width",
    &request.width.to_string(),
    "--height",
    &request.height.to_string(),
    "--steps",
    &request.steps.to_string(),
    "--seed",
    &request.seed.to_string(),
    "--",
    &request.model,
    &request.prompt,
  ]);
  eprintln!(
    "通过 Ollama CLI 生成图片：{}（日志：{}）……",
    request.model,
    artifacts.display()
  );
  if let Err(error) = ollama::execute(&mut command, artifacts, "image", args.timeout) {
    let retained = workspace.keep();
    return Err(error.context(format!("推理临时文件保留在 {}", retained.display())));
  }
  let images: Vec<PathBuf> = fs::read_dir(workspace.path())?
    .map(|entry| entry.map(|entry| entry.path()))
    .collect::<std::io::Result<Vec<_>>>()?
    .into_iter()
    .filter(|path| {
      path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    })
    .collect();
  let retained = workspace.keep();
  ensure!(
    images.len() == 1,
    "Ollama CLI 必须生成恰好一张 PNG，实际 {} 张；文件：{}",
    images.len(),
    retained.display()
  );
  let source = &images[0];
  verify(source, request.width, request.height)?;
  fs::hard_link(source, &request.output)
    .with_context(|| format!("无法发布 {}；不会覆盖已存在文件", request.output.display()))?;
  fs::remove_file(source)?;
  fs::remove_dir(retained)?;
  Ok(())
}

pub fn verify(output: &Path, width: u32, height: u32) -> Result<()> {
  let mut reader = image::ImageReader::open(output)
    .with_context(|| format!("推理未生成 {}", output.display()))?
    .with_guessed_format()?;
  ensure!(
    reader.format() == Some(image::ImageFormat::Png),
    "推理结果不是 PNG"
  );
  let mut limits = image::Limits::default();
  limits.max_image_width = Some(width);
  limits.max_image_height = Some(height);
  reader.limits(limits);
  let image = reader
    .decode()
    .context("推理结果不是可解码的图像或尺寸超出请求")?;
  ensure!(
    image.width() == width && image.height() == height,
    "图像尺寸不匹配：期望 {width}×{height}，实际 {}×{}",
    image.width(),
    image.height()
  );
  Ok(())
}
