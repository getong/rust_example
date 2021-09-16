use rusty_ulid::generate_ulid_bytes;
use rusty_ulid::generate_ulid_string;

use rusty_ulid::Ulid;
use std::str::FromStr;

fn main() {
    // println!("Hello, world!");

    // Generate a ULID string
    let ulid_string: String = generate_ulid_string();
    assert_eq!(ulid_string.len(), 26);

    // Generate ULID bytes
    let ulid_bytes: [u8; 16] = generate_ulid_bytes();
    assert_eq!(ulid_bytes.len(), 16);

    // Generate a ULID
    let ulid = Ulid::generate();

    // Generate a string for a ULID
    let ulid_string = ulid.to_string();

    // Create ULID from a string
    let result = Ulid::from_str(&ulid_string);

    println!("ulid_string:{}", ulid_string);

    assert_eq!(Ok(ulid), result);
}
