use base64::{alphabet, engine};
use base64::{engine::general_purpose, Engine as _};

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

fn main() {
    // println!("Hello, world!");
    basic_base64();
    customize_base64();
}
