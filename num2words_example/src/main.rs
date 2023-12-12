use num2words::*;

fn main() {
  // println!("Hello, world!");
  assert_eq!(
    Num2Words::new(42).lang(Lang::English).to_words(),
    Ok(String::from("forty-two"))
  );
  assert_eq!(
    Num2Words::new(42).ordinal().to_words(),
    Ok(String::from("forty-second"))
  );
  assert_eq!(
    Num2Words::new(42.01).currency(Currency::DOLLAR).to_words(),
    Ok(String::from("forty-two dollars and one cent"))
  );
}
