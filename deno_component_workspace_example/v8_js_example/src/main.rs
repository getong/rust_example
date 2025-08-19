use v8_js_example::create_user_token;

fn main() {
  println!("Starting stream_chat_token example...");
  let result = create_user_token("abc", "cde", "fgh");
  match result {
    Ok(token) => println!("token is {:?}", token),
    Err(e) => eprintln!("Error: {:?}", e),
  }
  println!("Done!");
}
