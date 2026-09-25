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
#[command(version, about = "MAI-UI 客户端：App 界面设计实验 / 截图定位")]
struct Args {
  /// design 请求生成 App HTML；grounding 定位已有截图
  #[arg(long, value_enum, default_value = "design")]
  mode: Mode,
  /// App 界面设计需求（design 模式）
  #[arg(
    long,
    default_value = "设计一个中文旅行规划 \
                     App「远行」的首页，呈现可交付的精致移动端视觉稿。暖白底色、深墨绿主色、\
                     橙色点缀。顶部有品牌、早安问候、头像和带未读圆点的通知按钮。主视觉是「大理 · \
                     去有风的地方」，用内联 SVG \
                     画出层叠苍山、洱海、太阳和蜿蜒路线，叠加「距出发还有 12 天」标签；行程日期 \
                     2026.10.07—10.10，4 天 3 晚，2 \
                     人同行，带查看行程主按钮和同行者头像缩略组。下方显示预算 ¥3,600、已订 2/3 \
                     项、目的地晴天 22°C 的紧凑信息条。「每日安排」带 Day 1—Day 4 日期切换，选中 \
                     Day 1（10 月 7 日），用时间线展示 09:30 大理古城漫步、14:00 \
                     洱海骑行两项安排，每项有小型 SVG \
                     缩略图、地点、时长、分类标签，之间标注交通距离。再加一张沙溪古镇推荐卡，\
                     含插画、评分 4.9、人均 ¥120 和收藏按钮；添加「出发前准备」进度 \
                     2/3，交通和住宿已完成、行李待整理。底部为首页、行程、收藏、我的四项导航，\
                     四个 Tab 必须能点击切换到四个不同内容页，并同步高亮当前项。\
                     行程页包含完整日程和预订详情，收藏页至少两张目的地卡片，我的页包含个人资料、\
                     旅行统计和设置列表；查看行程按钮也跳转行程页。使用单文件锚点和 CSS \
                     切换，默认显示首页，使用统一 SVG \
                     图标。主视觉优先，细节可向下滚动浏览；页面有清晰字号层次、细边框、克制阴影、\
                     充足留白，画面是 App 内部页面。"
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
    println!("App 界面设计实验：{}", args.brief);
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
