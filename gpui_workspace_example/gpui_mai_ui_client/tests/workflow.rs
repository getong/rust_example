use gpui_mai_ui_client::{
  document,
  generation::{Event, StreamDecoder},
  session::Session,
};
use serde_json::json;

const PAGE: &str =
  "<!doctype html><html><head><style>body{color:red}</style></head><body>测试界面</body></html>";

fn delta(content: &str) -> String {
  format!(
    "data: {}\n\n",
    json!({"choices": [{"delta": {"content": content}, "finish_reason": null}]})
  )
}

fn ending(reason: &str) -> String {
  format!(
    "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
    json!({"choices": [{"delta": {}, "finish_reason": reason}]}),
    json!({"choices": [], "usage": {"completion_tokens": 123}})
  )
}

#[test]
fn branching_only_sends_the_selected_ancestry_and_preserves_other_branches() {
  let mut session = Session::default();
  session
    .commit("初始设计".into(), PAGE.into(), None)
    .unwrap();
  session
    .commit("变成蓝色".into(), PAGE.replace("red", "blue"), None)
    .unwrap();
  session.back();
  assert_eq!(session.active, Some(0));
  session
    .commit("变成绿色".into(), PAGE.replace("red", "green"), None)
    .unwrap();
  assert_eq!(session.revisions.len(), 3);
  assert_eq!(session.revisions[2].parent, Some(0));
  assert!(session.revisions[1].html.contains("blue"));
  let request = session.request("增加侧边栏").unwrap();
  let messages = request["messages"].as_array().unwrap();
  assert_eq!(messages.len(), 6);
  assert_eq!(messages[1]["content"], "初始设计");
  assert_eq!(messages[3]["content"], "变成绿色");
  assert!(!request.to_string().contains("变成蓝色"));
  assert_eq!(messages[5]["content"], "增加侧边栏");
  session.select(Some(1)).unwrap();
  assert!(session.current().unwrap().html.contains("blue"));
  session.select(None).unwrap();
  assert_eq!(
    session.request("新方向").unwrap()["messages"]
      .as_array()
      .unwrap()
      .len(),
    2
  );
}

#[test]
fn failed_commit_never_changes_stable_version_or_prompt_library() {
  let mut session = Session::default();
  session.commit("初始".into(), PAGE.into(), None).unwrap();
  assert!(
    session
      .commit("失败".into(), "<html>incomplete".into(), None)
      .is_err()
  );
  assert_eq!(session.revisions.len(), 1);
  assert_eq!(session.active, Some(0));
  assert_eq!(session.prompts, ["初始"]);
  assert!(session.commit(" ".into(), PAGE.into(), None).is_err());
  assert!(session.select(Some(10)).is_err());
  assert_eq!(session.active, Some(0));
}

#[test]
fn reusable_prompts_are_trimmed_and_deduplicated() {
  let mut session = Session::default();
  session.remember_prompt("  调整配色  ");
  session.remember_prompt("调整配色");
  session.remember_prompt("\n ");
  assert_eq!(session.prompts, ["调整配色"]);
}

#[test]
fn session_round_trip_keeps_branch_and_settings() {
  let mut session = Session::default();
  session.settings.model = "test-model".into();
  session
    .commit("初始".into(), PAGE.into(), Some(42))
    .unwrap();
  session.commit("第二版".into(), PAGE.into(), None).unwrap();
  session.back();
  let root = std::env::temp_dir().join(format!("mai-ui-test-{}", std::process::id()));
  let path = root.join("session.json");
  session.save(&path).unwrap();
  session.save(&path).unwrap();
  let restored = Session::load(&path).unwrap();
  assert_eq!(restored.active, Some(0));
  assert_eq!(restored.revisions.len(), 2);
  assert_eq!(restored.current().unwrap().output_tokens, Some(42));
  assert_eq!(restored.settings.model, "test-model");
  assert!(!root.join("session.json.tmp").exists());
  std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_or_cyclic_sessions_are_rejected_without_panics() {
  let mut session = Session::default();
  session.commit("初始".into(), PAGE.into(), None).unwrap();
  session.revisions[0].parent = Some(0);
  assert!(session.validate().is_err());
  assert!(session.request("修改").is_err());
  session.revisions[0].parent = None;
  session.active = Some(99);
  assert!(session.validate().is_err());
}

#[test]
fn request_rejects_invalid_configuration_and_empty_prompt() {
  let mut session = Session::default();
  assert!(session.request("\n ").is_err());
  for endpoint in ["nonsense", "file:///tmp/example", "ftp://example.com"] {
    session.settings.endpoint = endpoint.into();
    assert!(session.request("design").is_err());
  }
  session.settings.endpoint = "http://localhost:11434/v1/chat/completions".into();
  session.settings.max_tokens = 0;
  assert!(session.request("design").is_err());
}

#[test]
fn decoder_handles_every_byte_boundary_including_unicode_and_usage() {
  let mut decoder = StreamDecoder::default();
  let stream = format!(": keepalive\r\n\r\n{}{}", delta(PAGE), ending("stop"));
  let mut content = String::new();
  for byte in stream.as_bytes() {
    for event in decoder.push(&[*byte]).unwrap() {
      if let Event::Delta {
        content: fragment, ..
      } = event
      {
        content.push_str(&fragment);
      }
    }
  }
  let output = decoder.finish().unwrap();
  assert_eq!(content, PAGE);
  assert_eq!(output.content, PAGE);
  assert_eq!(output.tokens, Some(123));
}

#[test]
fn decoder_supports_crlf_multiline_events_and_reasoning() {
  let mut decoder = StreamDecoder::default();
  let events = decoder.push(b"event: message\r\ndata: {\r\ndata: \"choices\":[{\"delta\":{\"reasoning_content\":\"thinking\"}}]}\r\n\r\n").unwrap();
  assert!(matches!(&events[0], Event::Delta { content, thinking_chars: 8 } if content.is_empty()));
  decoder.push(delta(PAGE).as_bytes()).unwrap();
  decoder.push(ending("stop").as_bytes()).unwrap();
  assert_eq!(decoder.finish().unwrap().content, PAGE);
}

#[test]
fn decoder_rejects_truncation_disconnect_and_invalid_content() {
  for reason in ["length", "content_filter", "tool_calls"] {
    let mut decoder = StreamDecoder::default();
    decoder.push(delta(PAGE).as_bytes()).unwrap();
    decoder.push(ending(reason).as_bytes()).unwrap();
    assert!(decoder.finish().is_err());
  }
  let mut decoder = StreamDecoder::default();
  decoder.push(delta(PAGE).as_bytes()).unwrap();
  assert!(decoder.finish().is_err());
  let mut decoder = StreamDecoder::default();
  decoder.push(delta("<html>partial").as_bytes()).unwrap();
  decoder.push(ending("stop").as_bytes()).unwrap();
  assert!(decoder.finish().is_err());
}

#[test]
fn decoder_reports_api_errors_and_malformed_events() {
  for bytes in [
    b"data: {\"error\":\"model missing\"}\n\n".as_slice(),
    b"data: not json\n\n",
  ] {
    assert!(StreamDecoder::default().push(bytes).is_err());
  }
}

#[test]
fn decoder_accepts_done_without_trailing_newline() {
  let mut decoder = StreamDecoder::default();
  decoder
    .push(format!("{}{}", delta(PAGE), ending("stop").trim_end()).as_bytes())
    .unwrap();
  assert!(decoder.finish().is_ok());
}

#[test]
fn document_accepts_fences_and_rejects_partial_pages() {
  for content in [
    PAGE.to_owned(),
    format!("```html\n{PAGE}\n```"),
    format!("```\n{PAGE}\n```"),
  ] {
    assert_eq!(document::validate(&content).unwrap(), PAGE);
  }
  for content in [
    "<html>broken</html>",
    "<answer>{coordinate:[1,2]}</answer>",
    "<!doctype html><html><head><style></style></head><body>",
  ] {
    assert!(document::validate(content).is_err());
  }
}

#[test]
fn preview_contains_generated_markup_only_inside_sandboxed_attribute() {
  let hostile = "</iframe><script>alert('test')</script><img src=\"https://example.com\">";
  let output = document::preview(hostile);
  assert!(output.contains("sandbox=\"\""));
  assert!(output.contains("Content-Security-Policy"));
  assert!(!output.contains("<script>"));
  assert!(!output.contains("<img"));
  assert!(output.contains("&lt;script&gt;"));
  assert!(output.contains("default-src 'none'"));
  assert!(output.contains("form-action 'none'"));
  assert!(output.contains("srcdoc=\"&lt;!doctype html&gt;"));
  assert_eq!(output.matches("<iframe ").count(), 1);
}
