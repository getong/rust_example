use stroming::*;

fn main() {
  // println!("Hello, world!");
  let mut store = MemoryStreamStore::new();
  let data = r#"{"test": "data"}"#.as_bytes().to_vec();
  let msg = Message {
    message_type: "TestMessage".to_owned(),
    data,
  };

  let _ = store.write_to_stream("TestStream-1", StreamVersion::NoStream, &[msg]);

  let (version, messages) = store.read_from_stream("TestStream-1", ReadDirection::Forwards);

  assert_eq!(version, StreamVersion::Revision(0));
  assert_eq!(messages.len(), 1);
  assert_eq!(messages[0].message_type, "TestMessage");
}
