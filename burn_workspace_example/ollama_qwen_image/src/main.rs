mod cli;
mod generation;
mod ollama;

use std::{
  fs,
  io::{self, BufRead, IsTerminal, Write},
  path::{Path, PathBuf},
  time::Instant,
};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde_json::json;

use cli::Args;

fn main() -> Result<()> {
  let mut args = Args::parse();
  args.validate()?;
  if args.ollama.is_relative() && args.ollama.components().count() > 1 {
    args.ollama = std::env::current_dir()?.join(&args.ollama);
  }
  if args.list_models {
    let artifacts = tempfile::tempdir()?;
    let mut command = ollama::command(&args);
    command.arg("list");
    let bytes = ollama::execute(&mut command, artifacts.path(), "list", args.timeout)?;
    io::stdout().write_all(&bytes)?;
    return Ok(());
  }
  if let Some(prompt) = &args.prompt {
    return generate(&args, prompt);
  }
  let stdin = io::stdin();
  let interactive = stdin.is_terminal();
  let mut lines = stdin.lock().lines();
  loop {
    if interactive {
      eprint!("提示词（Ctrl-D 退出）> ");
      io::stderr().flush()?;
    }
    let Some(line) = lines.next() else {
      break;
    };
    let prompt = line.context("无法读取标准输入")?;
    if prompt.trim().is_empty() {
      continue;
    }
    generate(&args, &prompt)?;
  }
  Ok(())
}

fn artifacts_path(output: &Path) -> PathBuf {
  output.with_extension("artifacts")
}

fn output_path(args: &Args) -> Result<PathBuf> {
  if let Some(output) = &args.output {
    return Ok(output.clone());
  }
  for sequence in 1..=u32::MAX {
    let output = PathBuf::from(format!("output-{sequence:04}.png"));
    if !output.exists() && !artifacts_path(&output).exists() {
      return Ok(output);
    }
  }
  anyhow::bail!("无法找到可用输出文件名，请使用 --output")
}

fn generate(args: &Args, original_prompt: &str) -> Result<()> {
  ensure!(!original_prompt.trim().is_empty(), "提示词不能为空");
  let start = Instant::now();
  let output = output_path(args)?;
  ensure!(
    !output.exists(),
    "输出已存在：{}，请换一个 --output",
    output.display()
  );
  let parent = output
    .parent()
    .filter(|path| !path.as_os_str().is_empty())
    .unwrap_or(Path::new("."));
  fs::create_dir_all(parent).with_context(|| format!("无法创建 {}", parent.display()))?;
  let parent = parent.canonicalize()?;
  let output = parent.join(output.file_name().context("输出文件名无效")?);
  let artifacts = artifacts_path(&output);
  fs::create_dir(&artifacts).with_context(|| {
    format!(
      "无法创建 {}；已有运行记录不会被覆盖，请换一个 --output",
      artifacts.display()
    )
  })?;
  eprintln!("运行记录：{}", artifacts.display());
  fs::write(artifacts.join("input.txt"), original_prompt)?;
  if !args.dry_run {
    generation::preflight(args, &artifacts)?;
  }
  let prompt = ollama::enhance(args, original_prompt, &artifacts)?;
  fs::write(artifacts.join("prompt.txt"), &prompt)?;
  let request = generation::Request::new(args, prompt, output.clone());
  let request_path = artifacts.join("generation.json");
  fs::write(&request_path, serde_json::to_vec_pretty(&request)?)?;
  if args.dry_run {
    eprintln!(
      "仅准备提示词和生成请求，未加载图像模型：{}",
      request_path.display()
    );
    return Ok(());
  }
  generation::execute(args, &request, &artifacts)?;
  generation::verify(&output, args.width, args.height)?;
  fs::write(
    artifacts.join("result.json"),
    serde_json::to_vec_pretty(&json!({
      "backend": request.backend,
      "model": request.model,
      "output": output,
      "width": args.width,
      "height": args.height,
      "seed": args.seed,
      "elapsed_seconds": start.elapsed().as_secs_f64(),
    }))?,
  )?;
  println!("{}", output.display());
  Ok(())
}
