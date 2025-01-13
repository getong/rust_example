use axum::extract::ws::Utf8Bytes;

#[tokio::main]
async fn main() {
  let word = "hello world";
  let utf8bytes = Utf8Bytes::from_static(word);
  let word_str = utf8bytes.as_str().to_string();

  let another_utf8bytes = Utf8Bytes::from(word_str);
  println!("another_utf8bytes {:?}", another_utf8bytes);
}
