use base64::{alphabet, engine, engine::general_purpose, Engine as _};

fn basic_decode() {
  let bytes = general_purpose::STANDARD
    .decode("aGVsbG8gd29ybGR+Cg==")
    .unwrap();
  println!("{:?}", bytes);

  // custom engine setup
  let bytes_url = engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD)
    .decode("aGVsbG8gaW50ZXJuZXR-Cg")
    .unwrap();
  println!("{:?}", bytes_url);
}

fn basic_base64() {
  let orig = b"data";
  let encoded: String = general_purpose::STANDARD_NO_PAD.encode(orig);
  assert_eq!("ZGF0YQ", encoded);
  assert_eq!(
    orig.as_slice(),
    &general_purpose::STANDARD_NO_PAD.decode(encoded).unwrap()
  );

  // or, URL-safe
  let encoded_url = general_purpose::URL_SAFE_NO_PAD.encode(orig);
  println!("encoded_url:{:?}", encoded_url);
}

fn customize_base64() {
  // bizarro-world base64: +/ as the first symbols instead of the last
  let alphabet =
    alphabet::Alphabet::new("+/ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789")
      .unwrap();

  // a very weird config that encodes with padding but requires no padding when decoding...?
  let crazy_config = engine::GeneralPurposeConfig::new()
    .with_decode_allow_trailing_bits(true)
    .with_encode_padding(true)
    .with_decode_padding_mode(engine::DecodePaddingMode::RequireNone);

  let crazy_engine = engine::GeneralPurpose::new(&alphabet, crazy_config);

  let encoded = crazy_engine.encode(b"abc 123");
  println!("encoded: {:?}", encoded);
}

fn base64_string_length() {
  let params = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzUxMiJ9.eyJpZCI6MjAsImNvbnN1bWVyIjoiMHhiZjNhMjg2YTQ3Nzk2N2ViZDg1MGNlZTJkYmRiZmE2ZTUzNWE5ZTY0IiwiaWF0IjoxNzIyODI2NzUzMjY2LCJleHAiOjE3MjM0MzE1NTMyOTl9.5b1pLX7TL8ng5yIcOoyIJg83zlY01aF1he81ZI_TNC2nl4ra0fTpLBKMjZKlyu-TagUwt5QRC8dn4jnH51pkEg";

  let raw = general_purpose::STANDARD.decode(params);

  if let Ok(raw) = raw {
    if raw.len() != 267 {
      println!("token not match, not 267");
    } else {
      println!("token match 267 bytes");
    }
  } else {
    println!("raw is not base64");
  }
}

fn main() {
  // println!("Hello, world!");
  basic_base64();
  customize_base64();
  basic_decode();

  base64_string_length();
}
