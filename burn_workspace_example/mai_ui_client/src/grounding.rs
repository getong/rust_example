use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde_json::{Value, json};

// MAI-UI 将自然语言描述映射到截图中的目标位置；客户端负责绘制标注。
// 保持输出仅含坐标，便于解析，也避免解释文本耗尽 128 token 的输出预算。
const GROUNDING_PROMPT: &str = concat!(
  "You are MAI-UI, a GUI grounding assistant. Your task is to connect the user's ",
  "natural-language request to the location of a visible UI element in the supplied screenshot. ",
  "Use the element's appearance, visible text, and surrounding layout to identify the target. ",
  "Base your answer only on this screenshot; do not assume a familiar app's usual layout. ",
  "Return the center of the requested element's visible clickable area. ",
  "For an app icon, return the center of the icon itself, not its text label. ",
  "Coordinates must be two integers [x, y] normalized to 0 through 999 over the entire \
   screenshot. ",
  "The origin is at the top-left; x increases to the right and y increases downward. ",
  "Return exactly <answer>{\"coordinate\": [x, y]}</answer>, replacing x and y with integers. ",
  "If the target is absent or cannot be identified unambiguously, ",
  "return exactly <answer>{\"coordinate\": null}</answer>. ",
  "Do not guess a location, execute a click, or claim that an action was completed. ",
  "Do not include explanations, Markdown, extra fields, or text outside the answer tags."
);

pub(crate) fn png_size(bytes: &[u8]) -> Result<(u32, u32)> {
  ensure!(
    bytes.len() >= 33 && bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
    "请使用包含完整 IHDR 的 PNG 截图"
  );
  ensure!(
    &bytes[8 .. 16] == b"\0\0\0\rIHDR",
    "PNG 缺少有效的 IHDR 数据块"
  );
  let width = u32::from_be_bytes(bytes[16 .. 20].try_into()?);
  let height = u32::from_be_bytes(bytes[20 .. 24].try_into()?);
  ensure!(width > 0 && height > 0, "PNG 尺寸不能为零");
  Ok((width, height))
}

pub(crate) fn request(target: &str, data_url: &str) -> Value {
  json!({
      "model": "MAI-UI-8B",
      "messages": [
          {"role": "system", "content": GROUNDING_PROMPT},
          {"role": "user", "content": [
              {"type": "text", "text": target},
              {"type": "image_url", "image_url": {"url": data_url}}
          ]}
      ],
      "temperature": 0, "max_tokens": 128, "stream": false
  })
}

pub(crate) fn prediction(response: &Value) -> Result<[u32; 2]> {
  let choice = response
    .pointer("/choices/0")
    .context("响应 choices 为空或缺失")?;
  ensure!(
    choice["finish_reason"] != "length",
    "输出被截断；已保存原始响应，不绘制不完整预测。"
  );
  let content = choice
    .pointer("/message/content")
    .and_then(Value::as_str)
    .context("响应缺少 message.content 字符串")?;
  let pattern = Regex::new(r"(?s)<answer>\s*(\{.*?\})\s*(?:</answer>|<answer>)")?;
  let matched = pattern
    .captures(content)
    .context("模型没有返回预期的 <answer> 格式，请查看 response.json。")?;
  let answer: Value = serde_json::from_str(&matched[1]).context("answer 中的 JSON 无效")?;
  let coordinate = answer
    .get("coordinate")
    .context("answer 缺少 coordinate 字段")?;
  if coordinate.is_null() {
    bail!("模型报告截图中没有该目标。");
  }
  let values = coordinate
    .as_array()
    .filter(|values| values.len() == 2)
    .with_context(|| format!("无效坐标：{coordinate}"))?;
  let mut point = [0; 2];
  for (slot, value) in point.iter_mut().zip(values) {
    *slot = value
      .as_u64()
      .filter(|&value| value <= 999)
      .with_context(|| format!("无效坐标：{coordinate}；必须是 0 到 999 的两个整数"))?
      as u32;
  }
  Ok(point)
}

pub(crate) fn pixel_position([x, y]: [u32; 2], (width, height): (u32, u32)) -> (u32, u32) {
  // Match Python round() and clamp the far edge inside the image.
  let scale = |v: u32, size: u32| {
    ((f64::from(v) / 999.0 * f64::from(size)).round_ties_even() as u32).min(size - 1)
  };
  (scale(x, width), scale(y, height))
}

fn escape_html(text: &str) -> String {
  text
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#x27;")
}

pub(crate) fn render_html(
  target: &str,
  data_url: &str,
  (width, height): (u32, u32),
  [x, y]: [u32; 2],
  (px, py): (u32, u32),
  elapsed: &str,
  content: &str,
) -> String {
  let title = escape_html(target);
  let content = escape_html(content);
  let elapsed = escape_html(elapsed);
  let left = f64::from(px) / f64::from(width) * 100.0;
  let top = f64::from(py) / f64::from(height) * 100.0;
  format!(
    include_str!("result.html"),
    title = title,
    content = content,
    elapsed = elapsed,
    left = left,
    top = top,
    data_url = data_url,
    x = x,
    y = y,
    px = px,
    py = py,
    width = width,
    height = height
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  fn response(content: &str) -> Value {
    json!({"choices": [{"message": {"content": content}, "finish_reason": "stop"}]})
  }
  #[test]
  fn accepts_both_answer_endings() {
    for end in ["</answer>", "<answer>"] {
      assert_eq!(
        prediction(&response(&format!(
          "text\n<answer>{{\"coordinate\":[616,829]}}{end}"
        )))
        .unwrap(),
        [616, 829]
      );
    }
  }
  #[test]
  fn rejects_invalid_predictions() {
    for coordinate in [
      "null", "[-1,0]", "[1000,0]", "[1.0,2]", "[true,2]", "[1]", "[1,2,3]", "\"1,2\"",
    ] {
      assert!(
        prediction(&response(&format!(
          "<answer>{{\"coordinate\":{coordinate}}}</answer>"
        )))
        .is_err()
      );
    }
    for content in [
      "<answer>{}</answer>",
      "<answer>{invalid}</answer>",
      "{\"coordinate\":[1,2]}",
    ] {
      assert!(prediction(&response(content)).is_err());
    }
    let mut truncated = response("<answer>{\"coordinate\":[1,2]}</answer>");
    truncated["choices"][0]["finish_reason"] = json!("length");
    assert!(prediction(&truncated).is_err());
    assert!(prediction(&json!({"choices": []})).is_err());
  }
  #[test]
  fn reads_screenshot_and_maps_coordinates() {
    let image = include_bytes!("../examples/official-screen.png");
    let size = png_size(image).unwrap();
    assert_eq!(pixel_position([0, 0], size), (0, 0));
    assert_eq!(pixel_position([999, 999], size), (size.0 - 1, size.1 - 1));
    assert_eq!(pixel_position([500, 500], (1, 1)), (0, 0));
    for len in [0, 8, 23, 32] {
      assert!(png_size(&image[.. len]).is_err());
    }
    let mut zero = image[.. 33].to_vec();
    zero[16 .. 20].fill(0);
    assert!(png_size(&zero).is_err());
  }
  #[test]
  fn escapes_untrusted_html() {
    let html = render_html(
      "<script>&\"'",
      "data:image/png;base64,AA==",
      (100, 200),
      [0, 0],
      (0, 0),
      "offline",
      "<answer>",
    );
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;&amp;&quot;&#x27;"));
    assert!(html.contains("&lt;answer&gt;"));
  }
}
