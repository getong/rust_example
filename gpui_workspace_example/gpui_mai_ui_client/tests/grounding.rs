use std::{
  io::Cursor,
  time::{SystemTime, UNIX_EPOCH},
};

use gpui_mai_ui_client::{
  document,
  generation::{ResponseFormat, StreamDecoder},
  grounding::{GroundingSession, Screenshot, parse_response},
  session::Settings,
};
use image::{DynamicImage, ImageFormat};
use serde_json::{Value, json};

fn screenshot() -> Screenshot {
  let mut bytes = Cursor::new(Vec::new());
  DynamicImage::new_rgb8(160, 100)
    .write_to(&mut bytes, ImageFormat::Png)
    .unwrap();
  Screenshot::from_bytes("测试截图.png".into(), bytes.get_ref()).unwrap()
}

const RESPONSE: &str = "<grounding_think>按钮在截图右上角。</grounding_think>\n<answer>{\"\
                        coordinate\":[999,0]}</answer>";

fn stream(content: &str, reason: &str) -> String {
  format!(
    "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
    json!({"choices":[{"delta":{"content":content}}]}),
    json!({"choices":[{"delta":{},"finish_reason":reason}],"usage":{"completion_tokens":40}})
  )
}

#[test]
fn reference_protocol_uses_999_and_pixels_stay_inside_image() {
  let result = parse_response(RESPONSE).unwrap();
  assert_eq!(result.coordinate, [999., 0.]);
  assert_eq!(result.normalized(), [1., 0.]);
  assert_eq!(result.pixels([160, 100]), [159, 0]);
  assert_eq!(result.explanation.as_deref(), Some("按钮在截图右上角。"));
  let center = parse_response("<answer>{\"coordinate\":[499.5,499.5]}</answer>").unwrap();
  assert_eq!(center.normalized(), [0.5, 0.5]);
  assert_eq!(center.pixels([161, 101]), [80, 50]);
}

#[test]
fn missing_ambiguous_invalid_or_not_found_results_are_rejected() {
  for response in [
    "<!doctype html><html></html>",
    "<answer>{\"coordinate\":[1,2]}",
    "<answer>not json</answer>",
    "<answer>{\"coordinate\":[1]}</answer>",
    "<answer>{\"coordinate\":[1,2,3]}</answer>",
    "<answer>{\"coordinate\":[-1,2]}</answer>",
    "<answer>{\"coordinate\":[1000,2]}</answer>",
    "<answer>{\"coordinate\":[\"1\",2]}</answer>",
    "<answer>{\"error\":\"target not found\"}</answer>",
    "<answer>{\"coordinate\":[1,2]}</answer><answer>{\"coordinate\":[3,4]}</answer>",
  ] {
    assert!(parse_response(response).is_err(), "{response}");
  }
}

#[test]
fn multimodal_request_matches_reference_and_does_not_include_html_ancestry() {
  let screenshot = screenshot();
  let request = screenshot
    .request(&Settings::default(), " 定位设置按钮 ")
    .unwrap();
  assert_eq!(request["messages"].as_array().unwrap().len(), 2);
  assert_eq!(request["messages"][0]["content"][0]["type"], "text");
  assert_eq!(request["messages"][1]["content"][0]["text"], "定位设置按钮");
  assert_eq!(request["messages"][1]["content"][1]["type"], "image_url");
  assert!(
    request["messages"][1]["content"][1]["image_url"]["url"]
      .as_str()
      .unwrap()
      .starts_with("data:image/png;base64,")
  );
  assert_eq!(request["temperature"], 0.);
  assert_eq!(request["stream"], true);
  assert_eq!(request["stream_options"]["include_usage"], true);
  assert!(screenshot.request(&Settings::default(), "  ").is_err());
}

#[test]
fn screenshots_are_decoded_and_reencoded_not_trusted_by_extension() {
  let screenshot = screenshot();
  assert_eq!(screenshot.dimensions(), [160, 100]);
  assert!(Screenshot::from_bytes("fake.png".into(), b"<script>alert(1)</script>").is_err());
  assert!(Screenshot::from_bytes("huge.png".into(), &vec![0; 8 * 1024 * 1024 + 1]).is_err());
  let result = parse_response(RESPONSE).unwrap();
  let preview = screenshot.preview(Some(&result));
  assert!(preview.contains("left:100%;top:0%"));
  assert!(!preview.contains("测试截图.png"));
  assert!(!screenshot.preview(None).contains("<span class=marker"));
  let wrapper = document::preview(&preview);
  assert!(wrapper.contains("sandbox=\"\""));
  assert!(wrapper.contains("img-src data:"));
  assert!(wrapper.contains("data:image/png;base64,"));
}

#[test]
fn jpeg_import_and_image_dimension_limits_are_enforced() {
  let mut bytes = Cursor::new(Vec::new());
  DynamicImage::new_rgb8(80, 60)
    .write_to(&mut bytes, ImageFormat::Jpeg)
    .unwrap();
  let image = Screenshot::from_bytes("截图.jpg".into(), bytes.get_ref()).unwrap();
  assert_eq!(image.dimensions(), [80, 60]);
  let mut bytes = Cursor::new(Vec::new());
  DynamicImage::new_rgb8(8193, 1)
    .write_to(&mut bytes, ImageFormat::Png)
    .unwrap();
  assert!(Screenshot::from_bytes("too-wide.png".into(), bytes.get_ref()).is_err());
}

#[test]
fn grounding_stream_accepts_tagged_utf8_but_preserves_html_validation() {
  let mut decoder = StreamDecoder::new(ResponseFormat::Grounding);
  for byte in stream(RESPONSE, "stop").as_bytes() {
    decoder.push(&[*byte]).unwrap();
  }
  let output = decoder.finish().unwrap();
  assert_eq!(output.content, RESPONSE);
  assert_eq!(output.tokens, Some(40));
  let mut decoder = StreamDecoder::default();
  decoder.push(stream(RESPONSE, "stop").as_bytes()).unwrap();
  assert!(decoder.finish().is_err());
  for (content, reason) in [(RESPONSE, "length"), ("<answer>{}</answer>", "stop")] {
    let mut decoder = StreamDecoder::new(ResponseFormat::Grounding);
    decoder.push(stream(content, reason).as_bytes()).unwrap();
    assert!(decoder.finish().is_err());
  }
  let mut decoder = StreamDecoder::new(ResponseFormat::Grounding);
  decoder
    .push(
      format!(
        "data: {}\n\n",
        json!({"choices":[{"delta":{"content":RESPONSE},"finish_reason":"stop"}]})
      )
      .as_bytes(),
    )
    .unwrap();
  assert!(decoder.finish().is_err());
}

#[test]
fn history_restores_snapshot_and_failed_prediction_does_not_replace_it() {
  let mut session = GroundingSession::default();
  assert!(
    session
      .commit("定位".into(), RESPONSE.into(), None)
      .is_err()
  );
  session.import(screenshot());
  session
    .commit("定位设置".into(), RESPONSE.into(), Some(40))
    .unwrap();
  assert!(
    session
      .commit(
        "定位不存在的按钮".into(),
        "<answer>{\"error\":\"not found\"}</answer>".into(),
        None
      )
      .is_err()
  );
  assert_eq!(session.steps().len(), 1);
  assert_eq!(session.selected().unwrap().instruction, "定位设置");
  session.import(screenshot());
  assert!(session.selected().is_none());
  let serialized = serde_json::to_value(&session).unwrap();
  assert_eq!(serialized["screenshots"].as_array().unwrap().len(), 1);
  let mut bytes = Cursor::new(Vec::new());
  DynamicImage::new_rgb8(80, 60)
    .write_to(&mut bytes, ImageFormat::Png)
    .unwrap();
  session.import(Screenshot::from_bytes("另一张截图.png".into(), bytes.get_ref()).unwrap());
  session
    .commit(
      "定位标题".into(),
      "<answer>{\"coordinate\":[0,0]}</answer>".into(),
      None,
    )
    .unwrap();
  assert_eq!(session.screenshot().unwrap().dimensions(), [80, 60]);
  session.select(0).unwrap();
  assert_eq!(session.screenshot().unwrap().dimensions(), [160, 100]);
  assert_eq!(session.selected().unwrap().result.coordinate, [999., 0.]);
  assert!(session.select(2).is_err());
  let serialized = serde_json::to_value(&session).unwrap();
  assert_eq!(serialized["screenshots"].as_array().unwrap().len(), 2);
}

#[test]
fn persisted_history_roundtrips_and_rejects_tampered_images_or_coordinates() {
  let path = std::env::temp_dir().join(format!(
    "mai-grounding-{}-{}.json",
    std::process::id(),
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos()
  ));
  let mut session = GroundingSession::load(&path, &Settings::default()).unwrap();
  session.import(screenshot());
  session.draft = "定位设置".into();
  session
    .commit(session.draft.clone(), RESPONSE.into(), Some(40))
    .unwrap();
  session.save(&path).unwrap();
  let loaded = GroundingSession::load(&path, &Settings::default()).unwrap();
  assert_eq!(loaded.screenshot().unwrap().dimensions(), [160, 100]);
  assert_eq!(loaded.selected().unwrap().tokens, Some(40));
  assert_eq!(loaded.draft, "定位设置");
  let original: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
  for (pointer, value) in [
    ("/screenshots/0/data_url", json!("javascript:alert(1)")),
    ("/screenshots/0/width", json!(123)),
    ("/steps/0/result/coordinate", json!([0, 0])),
    ("/current", json!(99)),
  ] {
    let mut corrupt = original.clone();
    *corrupt.pointer_mut(pointer).unwrap() = value;
    std::fs::write(&path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
    assert!(
      GroundingSession::load(&path, &Settings::default()).is_err(),
      "{pointer}"
    );
  }
  std::fs::remove_file(path).unwrap();
}
