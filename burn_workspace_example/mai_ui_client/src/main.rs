mod grounding;

use std::{
  fs,
  io::{self, Write},
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::Parser;
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(version, about = "截图定位演示：调用 MAI-UI API 并生成标注网页")]
struct Args {
  /// PNG 截图路径
  #[arg(long, default_value = "examples/official-screen.png")]
  image: PathBuf,
  /// 要定位的界面元素
  #[arg(long, default_value = "找到 Chrome 浏览器图标，返回图标中心的坐标。")]
  target: String,
  /// Chat Completions 接口地址
  #[arg(long, default_value = "http://localhost:30000/v1/chat/completions")]
  url: String,
  /// 输出目录
  #[arg(long, default_value = "demo-output")]
  output: PathBuf,
  /// 复用已有 response.json，不调用 API
  #[arg(long)]
  response: Option<PathBuf>,
}

fn main() -> Result<()> {
  run(Args::parse())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
  fs::write(path, serde_json::to_vec_pretty(value)?)
    .with_context(|| format!("无法写入 {}", path.display()))
}

fn run(args: Args) -> Result<()> {
  let raw_image =
    fs::read(&args.image).with_context(|| format!("无法读取截图 {}", args.image.display()))?;
  let size = grounding::png_size(&raw_image)?;
  let data_url = format!("data:image/png;base64,{}", STANDARD.encode(&raw_image));
  let payload = grounding::request(&args.target, &data_url);
  fs::create_dir_all(&args.output)
    .with_context(|| format!("无法创建输出目录 {}", args.output.display()))?;
  write_json(&args.output.join("request.json"), &payload)?;
  println!(
    "截图：{} ({} × {})\n任务：{}",
    args.image.display(),
    size.0,
    size.1,
    args.target
  );
  let (result, elapsed_label): (Value, String) = if let Some(path) = args.response {
    let bytes = fs::read(&path).with_context(|| format!("无法读取 {}", path.display()))?;
    (
      serde_json::from_slice(&bytes).context("已保存的响应不是有效 JSON")?,
      "复用已保存的响应，未再次调用 API".into(),
    )
  } else {
    println!("正在请求 {}，CPU 推理可能需要数分钟……", args.url);
    io::stdout().flush()?;
    let start = Instant::now();
    // Match the Python demo: bypass all proxy environment variables.
    let client = reqwest::blocking::Client::builder()
      .no_proxy()
      .timeout(Duration::from_secs(600))
      .build()
      .context("无法创建 HTTP 客户端")?;
    let response = client
      .post(&args.url)
      .json(&payload)
      .send()
      .context("请求失败；请检查服务和端口（超时为 600 秒）")?;
    let status = response.status();
    let body = response.text().context("无法读取 API 响应")?;
    if !status.is_success() {
      bail!("HTTP {status}: {body}\n429 表示模型忙，请等待当前请求结束。");
    }
    (
      serde_json::from_str(&body).context("API 响应不是有效 JSON")?,
      format!("{:.1} 秒", start.elapsed().as_secs_f64()),
    )
  };
  write_json(&args.output.join("response.json"), &result)?;
  let content = result
    .pointer("/choices/0/message/content")
    .and_then(Value::as_str)
    .context("响应缺少 choices[0].message.content 字符串；请查看 response.json")?;
  println!("耗时：{elapsed_label}\n模型原始输出：\n{content}");
  let coordinate = grounding::prediction(&result)?;
  let pixel = grounding::pixel_position(coordinate, size);
  let html = grounding::render_html(
    &args.target,
    &data_url,
    size,
    coordinate,
    pixel,
    &elapsed_label,
    content,
  );
  let output = args.output.join("result.html");
  fs::write(&output, html).with_context(|| format!("无法写入 {}", output.display()))?;
  println!(
    "归一化坐标：{coordinate:?}；截图像素：({}, {})\n可视化结果：{}",
    pixel.0,
    pixel.1,
    output.canonicalize()?.display()
  );
  Ok(())
}
