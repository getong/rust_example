//! `Cow` means "clone on write".
//!
//! It stores either `Borrowed(&T)` or `Owned(T::Owned)`. This is useful when a
//! function can usually return or pass through borrowed data, but sometimes must
//! allocate a changed value.

use std::{borrow::Cow, fmt};

/// Common case: static messages can be borrowed, dynamic messages must be owned.
fn modulo_3(input: u8) -> Cow<'static, str> {
  match input % 3 {
    0 => Cow::Borrowed("Remainder is 0"),
    1 => Cow::Borrowed("Remainder is 1"),
    remainder => Cow::Owned(format!("Remainder is {remainder}")),
  }
}

/// Avoids allocating a new `String` when the path already has the desired form.
fn normalize_path(path: &str) -> Cow<'_, str> {
  if !path.as_bytes().windows(2).any(|window| window == b"//") {
    return Cow::Borrowed(path);
  }

  let mut normalized = String::with_capacity(path.len());
  let mut previous_was_slash = false;

  for ch in path.chars() {
    if ch == '/' {
      if !previous_was_slash {
        normalized.push(ch);
      }
      previous_was_slash = true;
    } else {
      normalized.push(ch);
      previous_was_slash = false;
    }
  }

  Cow::Owned(normalized)
}

/// `to_mut()` is the write point: borrowed data is cloned only if redaction runs.
fn redact_password(mut message: Cow<'_, str>) -> Cow<'_, str> {
  const KEY: &str = "password=";

  let Some(value_start) = message.find(KEY).map(|start| start + KEY.len()) else {
    return message;
  };

  let value_end = message[value_start ..]
    .find([' ', '&'])
    .map_or(message.len(), |offset| value_start + offset);

  if value_start < value_end {
    message
      .to_mut()
      .replace_range(value_start .. value_end, "***");
  }

  message
}

/// A `Vec<Cow<str>>` can hold both static/default labels and runtime strings.
fn collect_tags<'a>(default_tags: &'a [&'a str], request_id: Option<String>) -> Vec<Cow<'a, str>> {
  let mut tags = Vec::with_capacity(default_tags.len() + usize::from(request_id.is_some()));
  tags.extend(default_tags.iter().copied().map(Cow::Borrowed));

  if let Some(request_id) = request_id {
    tags.push(Cow::Owned(request_id));
  }

  tags
}

/// `Cow` also works for slice-like data, not just strings.
fn only_even_numbers(numbers: &[i32]) -> Cow<'_, [i32]> {
  if numbers.iter().all(|number| number % 2 == 0) {
    Cow::Borrowed(numbers)
  } else {
    Cow::Owned(
      numbers
        .iter()
        .copied()
        .filter(|number| number % 2 == 0)
        .collect(),
    )
  }
}

fn print_str_cow(label: &str, value: &Cow<'_, str>) {
  match value {
    Cow::Borrowed(message) => println!("{label}: borrowed -> {message}"),
    Cow::Owned(message) => println!("{label}: owned    -> {message}"),
  }
}

fn print_slice_cow<T>(label: &str, value: &Cow<'_, [T]>)
where
  T: Clone + fmt::Debug,
{
  match value {
    Cow::Borrowed(numbers) => println!("{label}: borrowed -> {numbers:?}"),
    Cow::Owned(numbers) => println!("{label}: owned    -> {numbers:?}"),
  }
}

fn main() {
  println!("1) Return borrowed static text, allocate only for dynamic text");
  for number in 1 ..= 6 {
    let message = modulo_3(number);
    print_str_cow(&format!("modulo_3({number})"), &message);
  }

  println!("\n2) Normalize input only when it needs changes");
  for path in ["/api/v1/users", "/api//v1///users"] {
    let normalized = normalize_path(path);
    print_str_cow(path, &normalized);
  }

  println!("\n3) Mutate through to_mut, cloning borrowed text only on write");
  let safe_message = Cow::Borrowed("GET /health status=ok");
  let borrowed_secret = Cow::Borrowed("POST /login password=hunter2 ip=127.0.0.1");
  let owned_secret = Cow::Owned(format!(
    "POST /login user={} password={}",
    "alice", "123456"
  ));

  print_str_cow("safe message", &redact_password(safe_message));
  print_str_cow("borrowed secret", &redact_password(borrowed_secret));
  print_str_cow("owned secret", &redact_password(owned_secret));

  println!("\n4) Store borrowed and owned strings in the same collection");
  for tag in collect_tags(&["service", "cow-demo"], Some(format!("request-{}", 42))) {
    print_str_cow("tag", &tag);
  }

  println!("\n5) Cow also works with slices, not just str");
  let all_even = [2, 4, 6, 8];
  let mixed = [1, 2, 3, 4, 5, 6];

  print_slice_cow("all_even", &only_even_numbers(&all_even));
  print_slice_cow("mixed", &only_even_numbers(&mixed));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn modulo_3_borrows_static_messages_and_owns_dynamic_ones() {
    assert!(matches!(modulo_3(3), Cow::Borrowed(_)));
    assert!(matches!(modulo_3(4), Cow::Borrowed(_)));
    assert!(matches!(modulo_3(5), Cow::Owned(_)));
  }

  #[test]
  fn normalize_path_borrows_when_input_is_already_normalized() {
    let path = normalize_path("/api/v1/users");

    assert_eq!(path.as_ref(), "/api/v1/users");
    assert!(matches!(path, Cow::Borrowed(_)));
  }

  #[test]
  fn normalize_path_owns_when_input_needs_changes() {
    let path = normalize_path("/api//v1///users");

    assert_eq!(path.as_ref(), "/api/v1/users");
    assert!(matches!(path, Cow::Owned(_)));
  }

  #[test]
  fn redact_password_keeps_safe_messages_borrowed() {
    let message = redact_password(Cow::Borrowed("status=ok"));

    assert_eq!(message.as_ref(), "status=ok");
    assert!(matches!(message, Cow::Borrowed(_)));
  }

  #[test]
  fn redact_password_clones_borrowed_messages_when_redacting() {
    let message = redact_password(Cow::Borrowed("password=hunter2 ip=127.0.0.1"));

    assert_eq!(message.as_ref(), "password=*** ip=127.0.0.1");
    assert!(matches!(message, Cow::Owned(_)));
  }

  #[test]
  fn collect_tags_can_mix_borrowed_and_owned_values() {
    let tags = collect_tags(&["service"], Some("request-42".to_owned()));

    assert!(matches!(tags[0], Cow::Borrowed(_)));
    assert!(matches!(tags[1], Cow::Owned(_)));
    assert_eq!(tags[0].as_ref(), "service");
    assert_eq!(tags[1].as_ref(), "request-42");
  }

  #[test]
  fn only_even_numbers_borrows_when_everything_matches() {
    let numbers = [2, 4, 6];
    let even = only_even_numbers(&numbers);

    assert_eq!(even.as_ref(), &[2, 4, 6]);
    assert!(matches!(even, Cow::Borrowed(_)));
  }

  #[test]
  fn only_even_numbers_owns_when_filtering_is_needed() {
    let numbers = [1, 2, 3, 4];
    let even = only_even_numbers(&numbers);

    assert_eq!(even.as_ref(), &[2, 4]);
    assert!(matches!(even, Cow::Owned(_)));
  }
}
