use std::path::PathBuf;

use anyhow::{Result, ensure};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about = "通过 Ollama CLI 下载的图像模型生成 PNG；无需 Python")]
pub struct Args {
  /// 原始提示词；省略时逐行读取标准输入
  #[arg(long, allow_hyphen_values = true)]
  pub prompt: Option<String>,
  /// 输出 PNG；省略时自动选择未使用的 output-0001.png 等文件名
  #[arg(long)]
  pub output: Option<PathBuf>,
  /// Ollama 可执行文件；需要支持图像生成的版本
  #[arg(long, env = "OLLAMA_BIN", default_value = "ollama")]
  pub ollama: PathBuf,
  /// Ollama 服务地址；run.sh 默认使用项目独立服务 127.0.0.1:11435
  #[arg(long, env = "OLLAMA_HOST", default_value = "127.0.0.1:11434")]
  pub host: String,
  /// Ollama 图像模型名称，不是 Hugging Face 仓库名或文件目录
  #[arg(long, env = "OLLAMA_IMAGE_MODEL", default_value = "x/z-image-turbo")]
  pub image_model: String,
  /// 可选的提示词优化模型；省略时直接使用原始提示词
  #[arg(long, env = "OLLAMA_MODEL")]
  pub model: Option<String>,
  /// 跳过可选的提示词优化，但仍通过 Ollama 生成图片
  #[arg(long, conflicts_with = "response")]
  pub skip_ollama: bool,
  /// 复用保存的提示词 JSON，不调用文本模型
  #[arg(long)]
  pub response: Option<PathBuf>,
  /// 仅保存请求；没有 --model 时完全不启动 Ollama
  #[arg(long)]
  pub dry_run: bool,
  /// 列出当前服务的模型后退出
  #[arg(long)]
  pub list_models: bool,
  /// 每个 Ollama CLI 子进程的总超时（秒）
  #[arg(long, default_value_t = 7200, value_parser = clap::value_parser!(u64).range(1..))]
  pub timeout: u64,
  #[arg(long, default_value_t = 512, value_parser = clap::value_parser!(u32).range(64..=4096))]
  pub width: u32,
  #[arg(long, default_value_t = 512, value_parser = clap::value_parser!(u32).range(64..=4096))]
  pub height: u32,
  /// 0 使用图像模型默认步数
  #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..=1000))]
  pub steps: u32,
  /// Ollama 中 0 表示随机种子
  #[arg(long, default_value_t = 42, value_parser = clap::value_parser!(i64).range(0..))]
  pub seed: i64,
}

impl Args {
  pub fn validate(&self) -> Result<()> {
    ensure!(
      self
        .prompt
        .as_ref()
        .is_none_or(|prompt| !prompt.trim().is_empty()),
      "--prompt 不能为空"
    );
    ensure!(
      self.width.is_multiple_of(64) && self.height.is_multiple_of(64),
      "图像宽高必须是 64 的倍数"
    );
    for model in std::iter::once(&self.image_model).chain(self.model.iter()) {
      ensure!(
        !model.is_empty() && !model.starts_with('-') && !model.contains(char::is_whitespace),
        "Ollama 模型名称无效：{model:?}"
      );
    }
    ensure!(!self.host.trim().is_empty(), "--host 不能为空");
    if let Some(output) = &self.output {
      ensure!(
        self.prompt.is_some(),
        "逐行模式不接受 --output；使用自动编号避免覆盖"
      );
      ensure!(
        output
          .extension()
          .is_some_and(|extension| extension.eq_ignore_ascii_case("png")),
        "--output 必须是 .png 文件"
      );
    }
    Ok(())
  }
}
