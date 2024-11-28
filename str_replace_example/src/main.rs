// copy from [How do I replace specific characters idiomatically in Rust?](https://stackoverflow.com/questions/34606043/how-do-i-replace-specific-characters-idiomatically-in-rust)

fn main() {
  // println!("Hello, world!");
  let result: String = str::replace("Hello World!", "!", "?");
  // Equivalently:
  // result = "Hello World!".replace("!", "?");
  println!("{}", result); // => "Hello World?"
}
