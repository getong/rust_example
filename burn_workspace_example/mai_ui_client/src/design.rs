use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

// Experimental text-to-HTML task, not MAI-UI's official grounding/navigation task.
const DESIGN_PROMPT: &str = r#"Create a visual prototype of ONE app's internal screen from the user's design brief.
Output a complete HTML document starting with <!doctype html> and ending with </html>.
Include compact inline CSS in <style>. Use real Chinese interface copy, clear typography,
intentional spacing, a coherent palette, realistic content, and a prominent primary action.
Design for a 390px-wide, 844px-high mobile viewport with responsive width and no horizontal overflow.
This is the app itself: do not draw an Android launcher, app icon grid, device frame,
grounding marker, coordinates, explanatory report, or screenshot of an existing phone.
Use only HTML and CSS. No scripts, external resources, images, fonts, network requests, or Markdown fences.
Use CSS shapes or gradients if visual decoration is helpful. Keep the entire document under 1000 tokens.
Return the HTML artifact only, not instructions for making it."#;

pub(crate) fn request(brief: &str) -> Value {
  json!({
    "model": "MAI-UI-8B",
    "messages": [
      {"role": "system", "content": DESIGN_PROMPT},
      {"role": "user", "content": brief}
    ],
    "temperature": 0,
    "max_tokens": 2048,
    "stream": false
  })
}

pub(crate) fn document(response: &Value) -> Result<String> {
  ensure!(
    response
      .pointer("/choices/0/finish_reason")
      .and_then(Value::as_str)
      != Some("length"),
    "设计输出被截断；已保存 response.json，不生成不完整页面"
  );
  let content = response
    .pointer("/choices/0/message/content")
    .and_then(Value::as_str)
    .context("响应缺少 HTML 内容")?
    .trim();
  let html = content
    .strip_prefix("```html")
    .or_else(|| content.strip_prefix("```"))
    .and_then(|s| s.trim().strip_suffix("```"))
    .unwrap_or(content)
    .trim();
  let lower = html.to_ascii_lowercase();
  ensure!(
    lower.starts_with("<!doctype html>") && lower.ends_with("</html>") && lower.contains("<style"),
    "模型未返回完整的 HTML/CSS 界面；原始结果见 response.json。MAI-UI 的官方用途是 GUI \
     操作，设计生成属于实验任务"
  );
  // The preview is a static artifact. Browser-enforced policy prevents generated scripts
  // and external assets from executing/loading when the user opens it.
  let head = lower.find("<head>").context("HTML 缺少 <head> 标签")? + "<head>".len();
  let mut document = html.to_owned();
  document.insert_str(
    head,
    "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src \
     'unsafe-inline'; img-src data:; form-action 'none'; base-uri 'none'\">",
  );
  Ok(document)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn rejects_grounding_response_and_truncation() {
    let mut response = json!({"choices":[{"finish_reason":"stop","message":{"content":"<answer>{\"coordinate\":[1,2]}</answer>"}}]});
    assert!(document(&response).is_err());
    response["choices"][0]["message"]["content"] =
      json!("<!doctype html><html><head><style></style></head><body>旅行</body></html>");
    assert!(
      document(&response)
        .unwrap()
        .contains("Content-Security-Policy")
    );
    response["choices"][0]["finish_reason"] = json!("length");
    assert!(document(&response).is_err());
  }
}
