use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

// Experimental text-to-HTML task, not MAI-UI's official grounding/navigation task.
const DESIGN_PROMPT: &str = r##"You are a senior Unreal Engine 5 UI/UX designer designing interfaces for C++ game projects.
Create a polished, detailed, navigable visual prototype of ONE Unreal Engine 5 game interface
from the user's brief. The target implementation is native UE5 C++ with UMG or Slate.
HTML/CSS is only the offline visual preview format, not the target runtime or a Web Browser widget.

UNREAL ENGINE 5 DESIGN REQUIREMENTS
- Design player-facing game UI by default: main menu, HUD, inventory, quests, map, or settings
  as requested. Only design Unreal Editor tools or editor chrome if the brief explicitly asks for them.
- Compose layouts that can be implemented with UMG panels, anchors, alignment, padding, overlays,
  size constraints, and reusable widgets, or equivalent Slate layouts for C++ tools.
- Plan for DPI scaling, safe zones, different aspect ratios, and readable text over a game scene.
  Keep HUD elements near deliberate screen anchors and protect the central gameplay area.
- Define coherent normal, hover, keyboard-focus, selected, and disabled visual states.
  Make selection and focus distinct. Consider mouse/keyboard and gamepad navigation with visible
  input hints appropriate to the brief. Do not claim the browser preview implements UE input handling.
- Keep decorative scene art separate from UI panels so backgrounds, icons, and widget components
  can later be replaced by Unreal assets. Do not rely on browser-only visual tricks as core UI behavior.
- This is a design preview for Unreal Engine 5 C++, not a mobile app, website, or Blueprint tutorial.
  Do not add phone status bars, device frames, travel-app content, or mobile bottom tabs by default.

CONTENT AND VISUAL DIRECTION
- Follow the requested game genre, screen purpose, content, and art direction. Invent consistent,
  realistic Chinese sample content where needed; never use lorem ipsum, placeholder labels, or TODOs.
- Establish a clear hierarchy: screen title and player context, dominant task or selected item,
  supporting information, primary action, and concise input hints.
- Use purposeful game information such as character status, equipment attributes, quest progress,
  currency, or graphics/audio settings only where relevant. Avoid unrelated dashboard modules.
- Give the interface a distinctive visual identity with restrained accent colors, readable typography,
  clear spacing, fine borders, and intentional panel contrast. Avoid making every panel equally prominent.
- Use short inline SVG paths and simple CSS artwork for scene silhouettes, item icons, portraits,
  progress indicators, and consistent navigation icons. Limit each illustration to 12 shapes.
- Do not display C++ class names, implementation notes, or UMG/Slate terminology in player-facing UI.

PREVIEW NAVIGATION AND INTERACTION
- Choose 2-4 meaningful screens or panels from the brief; do not force unrelated destinations.
  If the brief asks for a single HUD or screen, implement that screen without inventing extra pages.
- For multiple screens, use sibling sections with class="ui-page" and meaningful unique IDs.
  Put persistent navigation outside them. Each navigation link must reference an existing section.
- Use CSS :target and :has() for local page switching without JavaScript. Show the default page
  when no destination is targeted; when another page is targeted, hide the default and show only
  that destination. Browser back/forward and direct hashes must work.
- Derive navigation selection styling from the targeted page, including the default-page state.
  Never hard-code a selected class. Give every destination distinct, populated content.
- Link primary navigation actions to actual sections. Use native details, radio buttons, checkboxes,
  and range inputs for preview interactions where appropriate. Label unavailable game actions as
  preview-only or disable them; do not pretend to launch a game, save settings, or load a backend.
- Provide visible :focus-visible styling and semantic controls that support keyboard access.
  Gamepad button hints are visual design references, not a claim of working gamepad support.

LAYOUT AND OUTPUT
- Default to a 1920 x 1080 landscape game viewport, with a usable scaled layout at 1280 x 720.
  Honor an explicitly requested platform, orientation, or resolution instead.
- Use a full-width game canvas with deliberate safe margins and flexible panel sizes. Preserve
  readable content on smaller windows; scroll long item/quest lists within their panels when needed.
  Avoid fixed heights on text-heavy content, clipped text, overlapping controls, and mobile-width shells.
- Include charset and viewport meta tags. Use semantic HTML, accessible control labels, aria-hidden
  on decorative SVGs, and adequate text/background contrast. Fixed overlays must not obscure content.
- Render completely offline with inline CSS and inline SVG. No JavaScript, remote URLs, external
  fonts, image downloads, external stylesheets, embedded frames, SVG scripts, or foreignObject.
  Never emit base64, data: URLs, <img>, SVG <image>, or CSS url().
- Return only one complete HTML document from <!doctype html> through </html>, with a literal
  <head> and inline <style>. No Markdown fences, explanatory text, or C++ source in this preview.
- Aim for 2500-4000 output tokens. Keep the style block under 100 compact rules, one rule per line,
  with no CSS comments, repeated selectors, redundant overrides, or unused classes. Close </style>
  and </head> early. Use shared component classes for the body components and artwork.
- Before returning, verify complete markup, readable game UI, working local navigation where present,
  visible focus and selection states, unobscured content, and a layout suitable for UE5 C++ UMG/Slate."##;

pub(crate) fn request(model: &str, brief: &str) -> Value {
  json!({
    "model": model,
    "messages": [
      {"role": "system", "content": DESIGN_PROMPT},
      {"role": "user", "content": brief}
    ],
    "temperature": 0.4,
    "frequency_penalty": 0.5,
    "max_tokens": 16384,
    "stream": false
  })
}

// Regenerate rather than append: a length-limited response may be a CSS repetition loop.
pub(crate) fn generate(
  payload: &Value,
  mut send: impl FnMut(&Value, usize) -> Result<Value>,
) -> Result<Value> {
  let first = send(payload, 1)?;
  if first
    .pointer("/choices/0/finish_reason")
    .and_then(Value::as_str)
    != Some("length")
  {
    return Ok(first);
  }
  eprintln!("模型达到输出 token 上限（不是等待超时）；已保存首轮响应，自动精简重试一次……");
  let mut retry = payload.clone();
  retry["messages"]
    .as_array_mut()
    .context("设计请求缺少 messages 数组")?
    .push(json!({
      "role": "user",
      "content": "The previous attempt exhausted its output budget. Generate a NEW complete compact document. Do not continue or repeat previous CSS. Preserve the Unreal Engine 5 C++ UMG/Slate design direction, landscape game layout, and all screens requested by the brief with working local navigation where applicable. Use at most 50 shared CSS rules, one rule per line, no comments, and short SVG paths. NEVER use base64, data URLs, img tags, SVG image tags or CSS url(); draw simple shapes directly. Close style within the first 1500 tokens. Keep the entire document under 6000 tokens and end with </html>. Simplify decoration before omitting required game UI content. Do not fall back to a mobile app layout."
    }));
  send(&retry, 2)
}

pub(crate) fn document(response: &Value) -> Result<String> {
  ensure!(
    response
      .pointer("/choices/0/finish_reason")
      .and_then(Value::as_str)
      != Some("length"),
    "设计输出达到 token 上限而被截断（不是请求超时）；已保存 response.json。可使用 --max-tokens \
     32768 重新生成，或精简 --brief；不会生成不完整页面"
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
  // Local hash links and CSS page switching work under this policy; scripts and
  // external assets remain blocked when the user opens the prototype.
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
  fn regenerates_truncation_without_feeding_repeated_css_back() {
    let payload = request("test", "four pages");
    let complete = json!({"choices":[{"finish_reason":"stop","message":{"content":
      "<!doctype html><html><head><style></style></head><body>完成</body></html>"}}]});
    let mut calls = 0;
    let result = generate(&payload, |sent, attempt| {
      calls += 1;
      assert_eq!(attempt, calls);
      assert_eq!(sent["model"], payload["model"]);
      assert_eq!(sent["max_tokens"], payload["max_tokens"]);
      if attempt == 1 {
        Ok(json!({"choices":[{"finish_reason":"length","message":{"content":"REPEATED_CSS"}}]}))
      } else {
        assert_eq!(sent["messages"].as_array().unwrap().len(), 3);
        assert!(!sent.to_string().contains("REPEATED_CSS"));
        Ok(complete.clone())
      }
    })
    .unwrap();
    assert_eq!(calls, 2);
    assert!(document(&result).is_ok());
  }

  #[test]
  fn bounds_retries_and_does_not_retry_success_or_transport_errors() {
    let payload = request("test", "four pages");
    for reason in ["stop", "length"] {
      let mut calls = 0;
      let result = generate(&payload, |_, _| {
        calls += 1;
        Ok(json!({"choices":[{"finish_reason":reason}]}))
      })
      .unwrap();
      assert_eq!(calls, if reason == "length" { 2 } else { 1 });
      assert_eq!(result["choices"][0]["finish_reason"], reason);
      if reason == "length" {
        assert!(document(&result).is_err());
      }
    }
    let mut calls = 0;
    assert!(
      generate(&payload, |_, _| {
        calls += 1;
        anyhow::bail!("connection timed out")
      })
      .is_err()
    );
    assert_eq!(calls, 1);
  }

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
