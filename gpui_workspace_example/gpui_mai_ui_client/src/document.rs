use anyhow::{Result, ensure};

pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

pub fn unfence(content: &str) -> &str {
  let content = content.trim();
  let content = content
    .strip_prefix("```html")
    .or_else(|| content.strip_prefix("```HTML"))
    .or_else(|| content.strip_prefix("```"))
    .unwrap_or(content);
  content.trim().strip_suffix("```").unwrap_or(content).trim()
}

pub fn validate(content: &str) -> Result<String> {
  ensure!(content.len() <= MAX_DOCUMENT_BYTES, "页面超过 4 MiB 限制");
  let html = unfence(content);
  let lower = html.to_ascii_lowercase();
  ensure!(
    lower.starts_with("<!doctype html>")
      && lower.ends_with("</html>")
      && lower.contains("<head>")
      && lower.contains("</head>")
      && lower.contains("<style")
      && lower.contains("</style>")
      && lower.contains("<body")
      && lower.contains("</body>"),
    "模型未返回完整 HTML/CSS 页面；当前版本未改变，请精简提示词或增加输出上限"
  );
  Ok(html.to_owned())
}

pub fn preview(content: &str) -> String {
  let content = unfence(content);
  let content = if content
    .get(.. "<!doctype html>".len())
    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype html>"))
  {
    &content["<!doctype html>".len() ..]
  } else {
    content
  };
  let escaped = content
    .replace('&', "&amp;")
    .replace('"', "&quot;")
    .replace('<', "&lt;")
    .replace('>', "&gt;");
  let policy = "&lt;meta http-equiv=&quot;Content-Security-Policy&quot; content=&quot;default-src \
                'none'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; base-uri \
                'none'&quot;&gt;";
  format!(
    "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
     content=\"width=device-width,initial-scale=1\"><title>MAI-UI \
     Preview</title><style>html,body,iframe{{margin:0;width:100%;height:100%;border:0;display:\
     block;background:#111827}}body{{overflow:hidden}}</style></head><body><iframe title=\"MAI-UI \
     设计预览\" sandbox=\"\" referrerpolicy=\"no-referrer\" srcdoc=\"&lt;!doctype \
     html&gt;{policy}{escaped}\"></iframe></body></html>"
  )
}

pub fn can_preview(content: &str) -> bool {
  let lower = content.to_ascii_lowercase();
  lower.contains("</style>") && lower.contains("<body")
}
