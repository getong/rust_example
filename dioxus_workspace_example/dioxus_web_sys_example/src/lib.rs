use js_sys::Math;
use wasm_bindgen::prelude::*;
use web_sys::{Element, console, window};

#[wasm_bindgen]
pub fn say_hello() {
  let random_number = Math::random();
  let message = format!("Hello from Rust! Random number: {}", random_number);

  // Log to the browser console
  console::log_1(&"Logging to console from Rust!".into());
  console::log_1(&format!("Generated random number: {}", random_number).into());

  // Show alert
  web_sys::window()
    .unwrap()
    .alert_with_message(&message)
    .unwrap();
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
  // Access the DOM window object
  let window = window().unwrap();
  let document = window.document().unwrap();

  // Get the button element by ID
  let button: Element = document.get_element_by_id("alert-btn").unwrap();

  // Set an event listener for the button click
  let closure = Closure::wrap(Box::new(move || {
    // Call the Rust function say_hello
    say_hello();
  }) as Box<dyn Fn()>);

  // Set an event listener for the button click
  button
    .dyn_ref::<web_sys::HtmlElement>()
    .unwrap()
    .set_onclick(Some(closure.as_ref().unchecked_ref()));

  // We need to keep the closure alive, so we store it in memory.
  closure.forget();

  Ok(())
}

pub fn add(left: u64, right: u64) -> u64 {
  left + right
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_works() {
    let result = add(2, 2);
    assert_eq!(result, 4);
  }
}
