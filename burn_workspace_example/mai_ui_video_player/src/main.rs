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

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Debug, Parser)]
#[command(version, about = "MAI-UI 客户端：视频播放器界面设计实验 / 截图定位")]
struct Args {
  /// design 生成视频播放器的 HTML 视觉预览；grounding 定位已有截图
  #[arg(long, value_enum, default_value = "design")]
  mode: Mode,
  /// 视频播放器界面设计需求（design 模式）
  #[arg(
    long,
    default_value = "设计中文桌面视频播放器「映刻 · FRAME」的高保真 UI，输出可离线打开的单文件 \
                     HTML/CSS 视觉预览，不实现真实视频播放。以 1440×900 横屏为基准，兼顾 1280×720 \
                     和窄窗口。采用石墨黑背景、分层深灰面板、克制的冰蓝色强调、\
                     细边框与清晰中文排版，避免把播放器做成数据仪表盘。左侧固定品牌和正在播放、\
                     媒体库、播放设置三项导航，用页面锚点切换并同步高亮。默认播放页以 16:9 \
                     视频画面为视觉中心，用简洁 SVG \
                     绘制山峦、湖泊和落日作为示例封面，展示纪录片「山海之间 · \
                     第一章」、导演陈默、4K / HDR / 24 FPS 信息。播放器下方展示示例时间 12:48 / \
                     48:32、进度滑块、上一集、播放/暂停、下一集、静音和音量滑块，提供倍速、字幕、\
                     画质选项以及全屏、画中画的禁用态设计。用小型「交互预览 · \
                     未加载媒体」标记说明状态，不声称这些控件能播放视频。\
                     右侧为「接下来播放」列表，含四集片名、时长、\
                     当前项高亮及列表内的真实锚点跳转，切换条目只演示选中与简介，不假装切换媒体。\
                     下方展示影片简介和章节清单。媒体库页展示最近观看、收藏和本地视频分组，\
                     包含六张有片名、时长、观看进度的封面卡片；卡片可跳转至播放页。\
                     播放设置页包含原生倍速与画质选择、音量滑块、字幕开关、\
                     字幕字号和深浅主题选项，仅演示控件状态，不声称持久保存或实际切换播放器主题。\
                     区分悬停、键盘焦点、选中和禁用状态，确保文字对比度、控件标签与键盘可访问性。\
                     窄屏时播放列表移至播放器下方，导航可换行，不裁切视频控制栏、不出现横向溢出。"
  )]
  brief: String,
  /// PNG 截图路径
  #[arg(long, default_value = "screenshot.png")]
  image: PathBuf,
  /// 要定位的界面元素
  #[arg(
    long,
    default_value = "找到视频播放器的播放/暂停按钮，返回按钮中心的坐标。"
  )]
  target: String,
  /// Ollama / OpenAI 兼容的 Chat Completions 完整接口地址
  #[arg(long, default_value = "http://localhost:11434/v1/chat/completions")]
  url: String,
  /// 服务端模型名称（必须与 ollama list 中的名称一致）
  #[arg(long, default_value = "Maternion/mai-ui:8b")]
  model: String,
  /// 设计模式的单次最大输出 token 数；与请求超时无关
  #[arg(long, default_value_t = 16384, value_parser = clap::value_parser!(u32).range(1..))]
  max_tokens: u32,
  /// 输出目录，默认位于 mai_ui_video_player 包内
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(match args.mode {
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
    println!("视频播放器界面设计实验：{}", args.brief);
    let mut payload = design::request(&args.model, &args.brief);
    payload["max_tokens"] = args.max_tokens.into();
    payload
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
      "正在请求 {}，模型 {}，最长等待 2 小时（7200 秒）；生成期间请保持客户端运行……",
      args.url, args.model
    );
    io::stdout().flush()?;
    let start = Instant::now();
    let client = reqwest::blocking::Client::builder()
      .no_proxy()
      .connect_timeout(Duration::from_secs(30))
      .timeout(REQUEST_TIMEOUT)
      .build()
      .context("无法创建 HTTP 客户端")?;
    let mut send = |payload: &Value, attempt: usize| -> Result<Value> {
      write_json(
        &output_dir.join(format!("request.attempt-{attempt}.json")),
        payload,
      )?;
      let response = client.post(&args.url).json(payload).send().context(
        "请求失败；连接超时为 30 秒，完整请求等待上限为 2 小时（7200 \
         秒）。请检查服务和端口；客户端超时不代表服务端推理已停止",
      )?;
      let status = response.status();
      let body = response
        .text()
        .context("无法读取 API 响应；完整请求等待上限为 2 小时（7200 秒）")?;
      if !status.is_success() {
        bail!("HTTP {status}: {body}\n429 表示模型忙，请等待当前请求结束。");
      }
      let result: Value = serde_json::from_str(&body).context("API 响应不是有效 JSON")?;
      write_json(
        &output_dir.join(format!("response.attempt-{attempt}.json")),
        &result,
      )?;
      write_json(&output_dir.join("response.json"), &result)?;
      Ok(result)
    };
    let result = match args.mode {
      Mode::Design => design::generate(&payload, &mut send)?,
      Mode::Grounding => send(&payload, 1)?,
    };
    (result, format!("{:.1} 秒", start.elapsed().as_secs_f64()))
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
  fn defaults_describe_video_player_and_validate_token_budget() {
    let args = Args::try_parse_from(["mai_ui_video_player"]).unwrap();
    assert!(matches!(args.mode, Mode::Design));
    assert!(args.brief.contains("映刻"));
    for feature in ["媒体库", "字幕", "16:9", "播放列表"] {
      assert!(args.brief.contains(feature));
    }
    assert!(!args.brief.contains("Unreal"));
    assert!(args.target.contains("播放/暂停"));
    assert_eq!(args.max_tokens, 16384);
    assert!(Args::try_parse_from(["mai_ui_video_player", "--max-tokens", "0"]).is_err());
  }

  #[test]
  fn replays_saved_design_without_contacting_server() {
    let unique = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let directory = std::env::temp_dir().join(format!(
      "mai-ui-video-player-{}-{unique}",
      std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let response_path = directory.join("saved.json");
    let output = directory.join("preview");
    let response = serde_json::json!({"choices": [{
      "finish_reason": "stop",
      "message": {"content": "<!doctype html><html><head><style>body{color:white}</style></head><body>视频播放器</body></html>"}
    }]});
    write_json(&response_path, &response).unwrap();
    let args = Args::try_parse_from([
      "mai_ui_video_player",
      "--response",
      response_path.to_str().unwrap(),
      "--output",
      output.to_str().unwrap(),
      "--url",
      "not-a-valid-url",
      "--max-tokens",
      "4096",
    ])
    .unwrap();
    let result = run(args);
    let html = fs::read_to_string(output.join("result.html"));
    let request = fs::read(output.join("request.json"));
    let saved = fs::read(output.join("response.json"));
    fs::remove_dir_all(&directory).unwrap();
    result.unwrap();
    let html = html.unwrap();
    assert!(html.contains("视频播放器"));
    assert!(html.contains("Content-Security-Policy"));
    let request: Value = serde_json::from_slice(&request.unwrap()).unwrap();
    assert_eq!(request["max_tokens"], 4096);
    assert_eq!(request["model"], "Maternion/mai-ui:8b");
    assert_eq!(
      serde_json::from_slice::<Value>(&saved.unwrap()).unwrap(),
      response
    );
  }

  #[test]
  fn ollama_defaults_and_custom_model_reach_both_requests() {
    let defaults = Args::try_parse_from(["mai_ui_video_player"]).unwrap();
    assert_eq!(defaults.url, "http://localhost:11434/v1/chat/completions");
    assert_eq!(defaults.model, "Maternion/mai-ui:8b");
    let custom = Args::try_parse_from([
      "mai_ui_video_player",
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
