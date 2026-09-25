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
#[command(
  version,
  about = "MAI-UI 客户端：Unreal Engine 5 C++ 界面设计实验 / 截图定位"
)]
struct Args {
  /// design 生成 UE5 C++ 界面的 HTML 视觉预览；grounding 定位已有截图
  #[arg(long, value_enum, default_value = "design")]
  mode: Mode,
  /// Unreal Engine 5 C++ 界面设计需求（design 模式）
  #[arg(
    long,
    default_value = "为 Unreal Engine 5 C++ \
                     开发的中文科幻探索游戏「星际边境」设计原生游戏界面，目标通过 UMG / Slate \
                     实现，当前输出单文件 HTML 视觉预览。以 1920×1080 横屏为基准，兼顾 \
                     1280×720、DPI \
                     缩放和安全区域。深蓝黑半透明面板、青色主色与少量琥珀色提醒，配合星球轮廓、\
                     轨道和星空的简洁 SVG \
                     背景。左侧为指挥中心、装备、任务、设置四项导航，能切换不同内容并同步高亮。\
                     指挥中心展示舰长林岚、等级 12、探索进度 68%、信用点 \
                     24,800，主任务「调查失联空间站」包含目标位置、\
                     危险等级和跳转任务页的查看任务按钮。装备页包含物品网格、稀有度、\
                     已装备标记及选中物品「脉冲步枪」的属性详情。任务页包含主线与支线任务、\
                     目标清单、奖励及完成进度。设置页包含画质选项、主音量滑块和字幕开关，使用原生 \
                     HTML 控件演示，不声称保存到游戏。保持统一的面板、按钮与图标风格，区分悬停、\
                     键盘焦点、选中和禁用状态，底部提供简洁的键鼠与手柄操作提示，\
                     手柄提示仅用于视觉设计。布局需适合后续用 UE5 C++ 的 UMG 控件树或 Slate \
                     组件实现；界面只展示玩家内容，不显示实现说明。"
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
  /// 设计模式的单次最大输出 token 数；与请求超时无关
  #[arg(long, default_value_t = 16384, value_parser = clap::value_parser!(u32).range(1..))]
  max_tokens: u32,
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
    println!("Unreal Engine 5 C++ 界面设计实验：{}", args.brief);
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
    // Match the Python demo: bypass all proxy environment variables.
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
