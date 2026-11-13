use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

const DESIGN_PROMPT: &str = r##"You are a senior UI/UX designer specializing in desktop video players.
Create ONE polished, detailed, navigable video player visual prototype from the user's brief.
The deliverable is an offline HTML/CSS design preview, NOT a working media player.

VIDEO PLAYER DESIGN REQUIREMENTS
- Make the video stage the dominant visual element, with a 16:9 aspect ratio unless requested otherwise.
  Keep the playback toolbar separate from the artwork so controls remain readable and unobscured.
- Design a coherent transport bar: previous, play/pause, next, elapsed/total time, seek slider,
  mute, volume, playback speed, subtitles, quality, picture-in-picture, and fullscreen as requested.
- All times, progress, resolution, HDR, and frame-rate metadata are illustrative sample content.
  Do not claim playback, seeking, file loading, persistence, fullscreen, or picture-in-picture works.
  Show a subtle Chinese preview/no-media status. Disable unavailable actions with meaningful labels.
- Use native selects, range inputs, checkboxes, radio buttons, and details for genuine local control
  state changes. A play/pause checkbox may demonstrate visual states only and must be labeled preview.
  Do not implement fake playback timers, custom keyboard shortcuts, or simulated backend operations.
- Include the playlist, media library, chapter list, film information, or settings requested in the brief.
  Do not invent unrelated analytics, game HUDs, quests, equipment, editor chrome, or dashboard metrics.

CONTENT AND VISUAL DIRECTION
- Follow the requested brand and art direction. Use realistic Chinese titles, descriptions, chapter
  names, durations, and metadata; never use lorem ipsum, TODOs, or empty placeholder cards.
- Establish hierarchy: brand/navigation, current film title, dominant video stage, transport controls,
  then supporting playlist and details. Distinguish the selected episode from keyboard focus.
- Use restrained accent colors, readable typography, generous spacing, fine borders, and deliberate
  panel contrast. Give the player a distinctive identity instead of making every panel equally prominent.
- Draw poster landscapes, thumbnails, and consistent controls with short inline SVG paths or CSS
  shapes. Limit each illustration to 12 shapes; reuse shared artwork styles for related thumbnails.

PREVIEW NAVIGATION AND INTERACTION
- Create only the screens requested, usually Now Playing, Media Library, and Playback Settings.
  For multiple pages, use sibling sections with class="ui-page" and unique IDs, and persistent navigation.
- Use CSS :target and :has() for page switching without JavaScript. Show Now Playing by default;
  show only the targeted page when another page is selected. Browser back/forward and direct hashes
  must work. Derive navigation highlights from the actual target, never a hard-coded selected class.
- Keep page switching scoped to .ui-page: an episode or chapter anchor inside the playback page must
  not accidentally hide that page. Every navigation or library link must reference an existing ID.
- Playlist links may target episode information to demonstrate selection, not switch actual media.
  Chapters may link to local descriptions but must not claim to seek. Distinct destinations need content.
- Give controls accessible names, semantic labels, visible :focus-visible styles, and clear hover,
  selected, and disabled states. Use real disabled buttons for unavailable media actions.

LAYOUT AND OUTPUT
- Default to a 1440 x 900 desktop window and remain usable at 1280 x 720 and narrower widths.
  Honor explicit viewport requirements. Use flexible grid/flex layouts with min-width:0 as needed.
- Keep the stage proportional. Move the playlist below the stage on narrow windows and allow the
  toolbar and navigation to wrap. Do not crop controls, overflow horizontally, or fix text-heavy heights.
- Include charset and viewport meta tags, semantic HTML, accessible control labels, aria-hidden on
  decorative SVGs, and adequate contrast. Do not cover the control bar with fixed overlays.
- Render completely offline with inline CSS and inline SVG. No JavaScript, remote URLs, external
  fonts, image downloads, stylesheets, frames, SVG scripts, foreignObject, video, or audio elements.
  Never emit base64, data: URLs, img tags, SVG image tags, or CSS url().
- Return only one complete HTML document from <!doctype html> through </html>, with a literal
  <head> and inline <style>. No Markdown fences, explanations, or implementation source code.
- Aim for 2500-4000 output tokens. Keep style under 100 shared compact rules, one per line, without
  comments, repeated selectors, redundant overrides, or unused classes. Close style and head early.
- Verify complete markup, all requested screens, consistent sample time/progress values, working
  local navigation, visible focus and selection, and unobscured player controls before returning."##;

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
      "content": "The previous attempt exhausted its output budget. Generate a NEW complete compact document. Do not continue or repeat previous CSS. Preserve the desktop video player direction, dominant 16:9 stage, playback toolbar, playlist, and every screen requested by the brief with working local navigation. Keep the preview/no-media status and disable unavailable playback actions. Use at most 50 shared CSS rules, one per line, no comments, and short SVG paths. NEVER use JavaScript, video/audio elements, base64, data URLs, img tags, SVG image tags or CSS url(); draw simple shapes directly. Close style within the first 1500 tokens. Keep the document under 6000 tokens and end with </html>. Simplify decoration before omitting required player controls."
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
  fn request_preserves_custom_brief_and_uses_video_player_direction() {
    let payload = request("custom-player", "浅色播放器与字幕设置");
    assert_eq!(payload["model"], "custom-player");
    assert_eq!(payload["messages"][1]["content"], "浅色播放器与字幕设置");
    assert_eq!(payload["stream"], false);
    let system = payload["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("desktop video players"));
    assert!(system.contains("NOT a working media player"));
    assert!(!system.contains("Unreal Engine"));
  }

  #[test]
  fn accepts_code_fences_and_rejects_incomplete_documents() {
    let html = "<!doctype html><html><head><style></style></head><body>播放器</body></html>";
    for content in [
      html.to_owned(),
      format!("```html\n{html}\n```"),
      format!("```\n{html}\n```"),
    ] {
      let response =
        json!({"choices": [{"finish_reason": "stop", "message": {"content": content}}]});
      let output = document(&response).unwrap();
      assert!(output.starts_with("<!doctype html>"));
      assert!(output.ends_with("</html>"));
      assert!(output.contains("default-src 'none'"));
    }
    for content in [
      "<!doctype html><html><head><style></style></head><body>",
      "<!doctype html><html><style></style><body></body></html>",
      "<!doctype html><html><head></head><body></body></html>",
    ] {
      let response = json!({"choices": [{"message": {"content": content}}]});
      assert!(document(&response).is_err());
    }
    assert!(document(&json!({"choices": []})).is_err());
  }

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
        let instruction = sent["messages"][2]["content"].as_str().unwrap();
        assert!(instruction.contains("desktop video player"));
        assert!(!instruction.contains("Unreal"));
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
