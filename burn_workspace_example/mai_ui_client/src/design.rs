use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

// Experimental text-to-HTML task, not MAI-UI's official grounding/navigation task.
const DESIGN_PROMPT: &str = r##"You are a senior mobile product designer and meticulous HTML/CSS craftsperson.
Create a polished, detailed, navigable prototype of ONE app from the user's brief.
The deliverable should look like a finished product screenshot, with intentional composition,
domain-specific content, custom visual assets, and carefully styled small details.

CONTENT AND COMPOSITION
- Follow the user's app category, content, and visual direction. Invent consistent, realistic
  Chinese sample data when details are missing. Never use lorem ipsum, placeholder labels, or TODOs.
- Establish a clear first-screen hierarchy: compact brand/greeting header, one visually dominant
  hero or primary task, supporting information, and a recognizable bottom navigation.
- Give the hero a distinctive visual composition: an original inline SVG illustration or layered
  CSS artwork, readable title, contextual metadata, a status badge, and one primary action.
- Include 3-5 purposeful supporting modules appropriate to the brief. Vary their composition:
  compact metric strip, segmented tabs, timeline rows, editorial cards, checklist, or mini map.
  Do not repeat identical large white cards for every section. Balance dense detail with breathing room.
- Make each row feel populated: primary label, secondary context, time/distance/price when relevant,
  a small visual or icon, a status or category label, and a clear action affordance.
- Use coherent sample names, dates, counts, and selected states throughout. Detail should explain
  the product, not become decorative filler. Write concise, natural Chinese UI copy.

VISUAL CRAFT
- Define CSS variables for background, surface, ink, muted text, accent, border, and spacing.
  Use a restrained palette with one dominant color and one sparing contrasting accent.
- Use a deliberate type scale: hero 28-34px, section headings 18-22px, body 14-16px, metadata 11-13px.
  Give headings strong weight, metadata adequate contrast, and Chinese body text comfortable line height.
- Use a consistent 4/8px spacing rhythm, 20px page gutters, 12-20px section gaps, and 12-24px corner radii.
  Keep related labels close together and leave larger gaps between unrelated groups.
- Add subtle 1px borders, restrained layered shadows, tinted surfaces, badge backgrounds,
  progress indicators, overlapping avatar initials, and fine dividers where the content calls for them.
- Draw consistent 20-24px inline SVG line icons using currentColor, viewBox, and matching stroke widths.
  Draw actual SVG paths/shapes; do not depend on icon fonts, emoji, missing images, or external assets.
- Build at least one substantial subject-specific illustration: e.g. mountains, sun, water and a
  route for travel; a small trend chart for finance; a progress visualization for a task app.
  Use layered shapes with a clear focal point. Keep labels and controls outside artwork legible.
- Style the selected tab and active navigation item clearly. Provide button pressed/hover states
  and visible keyboard focus using CSS. Bottom navigation must actually switch visible pages.

REQUIRED WORKING BOTTOM NAVIGATION (HIGHEST PRIORITY)
- Deliver all four pages in the same HTML file: 首页 (home), 行程 (trips), 收藏 (saved), 我的 (profile).
  Adapt the labels/content for other app categories while retaining these four stable page IDs.
- Use four sibling sections with class="app-page" and unique ids home, trips, saved, profile.
  Put the persistent bottom <nav aria-label="主导航"> outside those sections.
- Each navigation item MUST be a real anchor: <a href="#home">, <a href="#trips">,
  <a href="#saved">, <a href="#profile">, containing its icon and visible text.
  Never use inert divs, href="#", disabled buttons, onclick, or links to missing HTML files.
- Implement routing with CSS :target and :has(), with no JavaScript. Use these exact visibility rules:
  .app-page { display:none; }
  #home { display:block; }
  body:has(#trips:target, #saved:target, #profile:target) #home { display:none; }
  #trips:target, #saved:target, #profile:target { display:block; }
  Do not override these rules with later display declarations. Opening the file without a hash
  shows home; clicking each link shows only its matching page. Direct hashes and browser back/forward work.
- Style the active link using body:has(#trips:target) nav a[href="#trips"] and equivalent
  selectors for the other pages. Default home is active when body:not(:has(.app-page:target)).
  Apply the active foreground, background, and icon styling together. Never hard-code a selected class.
- Each destination must contain distinct, populated content, not a heading-only placeholder:
  trips: trip summary, dated itinerary and booking details; saved: at least two saved destination
  cards with category, price and rating; profile: avatar/name, travel statistics and settings rows.
  Home retains the detailed hero and supporting modules requested in the brief.
- Make the home primary "查看行程" action an <a href="#trips"> link too. Preserve the same
  visual system and navigation across pages. Reserve bottom space on EVERY page for the fixed nav.
- Check all four links by following their href values mentally: each must resolve to a unique,
  nonempty section, hide the other three pages, and update the selected navigation appearance.

LAYOUT AND IMPLEMENTATION
- Design for a 390 x 844 mobile viewport. Include charset and viewport meta tags.
  Use a centered app shell with width:100% and max-width:430px; support widths down to 320px.
  The first viewport should contain the header, hero, and the beginning of useful supporting content.
- Allow natural vertical scrolling for remaining details. Never compress all content into 844px,
  crop text, or use fixed heights on text-heavy sections. Use min-width:0 on shrinking flex/grid children.
- If navigation is fixed, center it to the same width as the shell and reserve enough bottom padding
  for its full height plus env(safe-area-inset-bottom). It must not cover the final content row.
- Use semantic HTML, buttons for actions, accessible icon labels, and aria-hidden on decorative SVGs.
  Keep primary tap targets at least 44px. Use anchors for navigation. No JavaScript or claims of
  working backend functionality; local page navigation must work without a server.
- Everything must render offline: inline CSS and inline SVG only; no remote URLs, external fonts,
  image downloads, scripts, external stylesheets, SVG scripts, foreignObject, or embedded frames.
  Never emit base64, data: URLs, <img>, SVG <image>, or CSS url(). Draw illustrations directly
  with short <svg> paths, circles and rectangles, at most 12 shapes per illustration.
- This is an app screen, not an Android launcher, device frame, report, coordinate annotation, or landing page.

OUTPUT CONTRACT
Return only one complete HTML document from <!doctype html> through </html>, with a literal <head>
and inline <style>. No Markdown fences or explanatory text. Aim for 2500-4000 output tokens.
CRITICAL: keep the style block under 100 lines, with one compact rule per line. Declare each selector
only once. No CSS comments, repeated rules, redundant overrides, or unused classes. Close </style>
and </head> early, then spend most of the output on actual body components and SVG artwork.
Use shared component classes to fit all requested details into a complete document.
Before returning, check that all tags close, the illustration exists, text remains readable,
the four bottom links switch populated pages, navigation cannot obscure content, and the document is complete."##;

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
      "content": "The previous attempt exhausted its output budget. Generate a NEW complete compact document. Do not continue or repeat previous CSS. Prioritize four populated pages and working navigation. Use at most 50 shared CSS rules, one rule per line, no comments, and short SVG paths. NEVER use base64, data URLs, img tags, SVG image tags or CSS url(); draw simple shapes directly. Close style within the first 1500 tokens. Keep the entire document under 6000 tokens and end with </html>. Simplify decoration before omitting any page."
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
