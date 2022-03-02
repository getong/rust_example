use std::fs::File;
use std::io::Write;
use std::time::SystemTime;
use totp_rs::{Algorithm, TOTP};

fn main() {
    // println!("Hello, world!");
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, "supersecret");
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let url = totp.get_url("user@example.com", "my-org.com");
    println!("{}", url);
    let token = totp.generate(time);
    println!("{}", token);

    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, "supersecret");
    if let Ok(code) = totp.get_qr("user@example.com", "my-org.com") {
        // println!("{}", code);
        if let Ok(mut file) = File::create("qr_code.txt") {
            file.write_all(&code.as_bytes()).unwrap();
        }
    }
}
