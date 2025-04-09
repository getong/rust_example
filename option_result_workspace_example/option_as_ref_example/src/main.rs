fn main() {
  // println!("Hello, world!");

  // std::option::Option::as_ref()
  // Converts from &Option<T> to Option<&T>.

  let text: Option<String> = Some("Hello, world!".to_string());
  println!("text is {:?}", text);
  // First, cast `Option<String>` to `Option<&String>` with `as_ref`,
  // then consume *that* with `map`, leaving `text` on the stack.
  let text_length: Option<usize> = text.as_ref().map(|s| s.len());
  println!(
    "still can print text: {:?}, text_length: {:?}",
    text, text_length
  );

  let a = Some("".to_string());
  let a1 = change_empty_string_to_none(&a);
  println!("a1 is {:?}", a1);

  let b = None;
  let b1 = change_empty_string_to_none(&b);
  println!("b1 is {:?}", b1);
}

fn change_empty_string_to_none(s: &Option<String>) -> Option<String> {
  s.as_ref()
    .map(|s| s.trim())
    .filter(|s| !s.is_empty())
    .map(|s| s.to_string())
}
