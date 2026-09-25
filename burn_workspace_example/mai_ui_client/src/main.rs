mod design;
mod grounding;

use std::{
  fs,
  io::{self, Write},
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Parser, ValueEnum};
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(version, about = "MAI-UI 客户端：App 界面设计实验 / 截图定位")]
struct Args {
  /// design 请求生成 App HTML；grounding 定位已有截图
  #[arg(long, value_enum, default_value = "design")]
  mode: Mode,
  /// App 界面设计需求（design 模式）
  #[arg(
    long,
    default_value = "设计一个中文旅行规划 \
                     App「远行」的首页。包含品牌栏、下一段旅程的目的地和日期、查看行程主按钮、\
                     两条每日安排及底部导航。暖白底色、深墨绿主色、橙色点缀，\
                     清晰的字号层次和充足留白。画面是 App 内部页面，不是手机桌面。"
  )]
  brief: String,
  /// PNG 截图路径
  #[arg(long, default_value = "examples/official-screen.png")]
  image: PathBuf,
  /// 要定位的界面元素
  #[arg(long, default_value = "找到 Chrome 浏览器图标，返回图标中心的坐标。")]
  target: String,
  /// Ollama / OpenAI 兼容的 Chat Completions 完整接口地址
  #[arg(long, default_value = "http://localhost:11434/v1/chat/completions")]
  url: String,
  /// 服务端模型名称（必须与 ollama list 中的名称一致）
  #[arg(long, default_value = "Maternion/mai-ui:8b")]
  model: String,
  /// 输出目录
  #[arg(long)]
  output: Option<PathBuf>,
  /// 复用已有 response.json，不调用 API
  #[arg(long)]
  response: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
  Design,
  Grounding,
}

fn main() -> Result<()> {
  run(Args::parse())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
  fs::write(path, serde_json::to_vec_pretty(value)?)
    .with_context(|| format!("无法写入 {}", path.display()))
}

fn run(args: Args) -> Result<()> {
  let output_dir = args.output.unwrap_or_else(|| {
    PathBuf::from(match args.mode {
      Mode::Design => "design-output",
      Mode::Grounding => "demo-output",
    })
  });
  let screenshot = match args.mode {
    Mode::Design => None,
    Mode::Grounding => {
      let bytes =
        fs::read(&args.image).with_context(|| format!("无法读取截图 {}", args.image.display()))?;
      Some((
        grounding::png_size(&bytes)?,
        format!("data:image/png;base64,{}", STANDARD.encode(bytes)),
      ))
    }
  };
  let payload = if let Some((_, data_url)) = &screenshot {
    grounding::request(&args.model, &args.target, data_url)
  } else {
    println!("App 界面设计实验：{}", args.brief);
    design::request(&args.model, &args.brief)
  };
  fs::create_dir_all(&output_dir)
    .with_context(|| format!("无法创建输出目录 {}", output_dir.display()))?;
  write_json(&output_dir.join("request.json"), &payload)?;
  let (result, elapsed_label): (Value, String) = if let Some(path) = args.response {
    let bytes = fs::read(&path).with_context(|| format!("无法读取 {}", path.display()))?;
    (
      serde_json::from_slice(&bytes).context("已保存的响应不是有效 JSON")?,
      "复用已保存的响应，未再次调用 API".into(),
    )
  } else {
    println!(
      "正在请求 {}，模型 {}，推理可能需要数分钟……",
      args.url, args.model
    );
    io::stdout().flush()?;
    let start = Instant::now();
    // Match the Python demo: bypass all proxy environment variables.
    let client = reqwest::blocking::Client::builder()
      .no_proxy()
      .timeout(Duration::from_secs(1800))
      .build()
      .context("无法创建 HTTP 客户端")?;
    let response = client
      .post(&args.url)
      .json(&payload)
      .send()
      .context("请求失败；请检查服务和端口（超时为 1800 秒）")?;
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
  write_json(&output_dir.join("response.json"), &result)?;
  let content = result
    .pointer("/choices/0/message/content")
    .and_then(Value::as_str)
    .context("响应缺少 choices[0].message.content 字符串；请查看 response.json")?;
  println!("耗时：{elapsed_label}\n模型原始输出：\n{content}");
  let html = if let Some((size, data_url)) = screenshot {
    let coordinate = grounding::prediction(&result)?;
    let pixel = grounding::pixel_position(coordinate, size);
    println!(
      "归一化坐标：{coordinate:?}；截图像素：({}, {})",
      pixel.0, pixel.1
    );
    grounding::render_html(
      &args.target,
      &data_url,
      size,
      coordinate,
      pixel,
      &elapsed_label,
      content,
    )
  } else {
    design::document(&result)?
  };
  let output = output_dir.join("result.html");
  fs::write(&output, html).with_context(|| format!("无法写入 {}", output.display()))?;
  println!("可视化结果：{}", output.canonicalize()?.display());
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ollama_defaults_and_custom_model_reach_both_requests() {
    let defaults = Args::try_parse_from(["mai_ui_client"]).unwrap();
    assert_eq!(defaults.url, "http://localhost:11434/v1/chat/completions");
    assert_eq!(defaults.model, "Maternion/mai-ui:8b");
    let custom = Args::try_parse_from([
      "mai_ui_client",
      "--model",
      "custom:latest",
      "--url",
      "http://localhost:30000/v1/chat/completions",
    ])
    .unwrap();
    assert_eq!(custom.url, "http://localhost:30000/v1/chat/completions");
    let image = "data:image/png;base64,AA==";
    let grounding = grounding::request(&custom.model, &custom.target, image);
    let design = design::request(&custom.model, &custom.brief);
    for request in [&grounding, &design] {
      assert_eq!(request["model"], "custom:latest");
      assert_eq!(request["stream"], false);
    }
    assert_eq!(
      grounding["messages"][1]["content"][1]["image_url"]["url"],
      image
    );
  }
}
