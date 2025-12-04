use loro::{ExportMode, LoroDoc, ToJson};
use serde_json::json;

fn main() {
  let doc = LoroDoc::new();
  let text = doc.get_text("text");
  text.insert(0, "Hello world!").unwrap();
  let bytes = doc.export(ExportMode::all_updates()).unwrap();
  let doc_b = LoroDoc::new();
  doc_b.import(&bytes).unwrap();
  assert_eq!(doc.get_deep_value(), doc_b.get_deep_value());
  let text_b = doc_b.get_text("text");
  text_b.mark(0 .. 5, "bold", true).unwrap();
  doc
    .import(&doc_b.export(ExportMode::updates(&doc.oplog_vv())).unwrap())
    .unwrap();
  assert_eq!(
    text.get_richtext_value().to_json_value(),
    json!([
        { "insert": "Hello", "attributes": {"bold": true} },
        { "insert": " world!" },
    ])
  );
}
