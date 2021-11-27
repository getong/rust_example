use pwhash::{bcrypt, bsdi_crypt, md5_crypt, sha1_crypt, sha256_crypt, unix_crypt};
use std::env;

fn main() {
    let mut password = "password";
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        password = args[1].as_str();
    }
    println!("Password:\t{:}", password);
    let mut h_new = bcrypt::hash(password).unwrap();
    println!("\nBcrypt:\t\t{:}", h_new);
    h_new = bsdi_crypt::hash(password).unwrap();
    println!("BSDI Crypt:\t{:}", h_new);
    h_new = md5_crypt::hash(password).unwrap();
    println!("MD5 Crypt:\t{:}", h_new);
    h_new = sha1_crypt::hash(password).unwrap();
    println!("SHA1 Crypt:\t{:}", h_new);
    h_new = sha256_crypt::hash(password).unwrap();
    println!("SHA-256 Crypt:\t{:}", h_new);
    h_new = unix_crypt::hash(password).unwrap();
    println!("Unix crypt:\t{:}", h_new);
    //    let rtn=bcrypt::verify(password, h);
    //   println!("{:?}",rtn);
}
