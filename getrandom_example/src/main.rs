fn get_random_buf() -> Result<[u8; 32], getrandom::Error> {
  let mut buf = [0u8; 32];
  getrandom::getrandom(&mut buf)?;
  Ok(buf)
}

fn main() {
  // println!("Hello, world!");
  match get_random_buf() {
    Ok(buf) => println!("buf is {:?}", buf),
    Err(_) => println!("error"),
  }
}
