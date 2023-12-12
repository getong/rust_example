use std::borrow::Cow;

fn modulo_3(input: u8) -> Cow<'static, str> {
  match input % 3 {
    0 => "Remainder is 0".into(),
    1 => "Remainder is 1".into(),
    remainder => format!("Remainder is {}", remainder).into(),
  }
}

fn main() {
  for number in 1..=6 {
    match modulo_3(number) {
      Cow::Borrowed(message) => println!(
        "{} went in. The Cow is borrowed with this message: {}",
        number, message
      ),
      Cow::Owned(message) => println!(
        "{} went in. The Cow is owned with this message: {}",
        number, message
      ),
    }
  }
}
