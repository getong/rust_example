//src/main.rs
mod jwt_sign;
use jwt_sign::create_jwt;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

use jwt_simple::prelude::*;
fn main() {
    let key = HS256Key::generate();
    let byte_data = key.to_bytes();

    let f = File::create("key").expect("Unable to create  file");
    let mut f = BufWriter::new(f);
    f.write_all(&byte_data).expect("Unable to write data");

    print!("{}", create_jwt("someone@gmail.com".to_string()));
}
