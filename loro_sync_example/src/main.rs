use loro::{LoroDoc, ToJson};
use serde_json::json;

fn main() {
  let doc = LoroDoc::new();
  let text = doc.get_text("text");
  text.insert(0, "Hello world!").unwrap();
  let bytes = doc.export_from(&Default::default());
  let doc_b = LoroDoc::new();
  doc_b.import(&bytes).unwrap();
  assert_eq!(doc.get_deep_value(), doc_b.get_deep_value());
  let text_b = doc_b.get_text("text");
  text_b.mark(0 .. 5, "bold", true).unwrap();
  doc.import(&doc_b.export_from(&doc.oplog_vv())).unwrap();
  assert_eq!(
    text.to_delta().to_json_value(),
    json!([
        { "insert": "Hello", "attributes": {"bold": true} },
        { "insert": " world!" },
    ])
  );
}
