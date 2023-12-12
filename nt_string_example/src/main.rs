use nt_string::unicode_string::NtUnicodeStr;
use nt_string::unicode_string::NtUnicodeString;

use u16cstr::{u16cstr, u16str};

fn subfunction(str_ref: &NtUnicodeStr) {
  println!("Hello from subfunction with \"{str_ref}\".");
}

fn main() {
  // println!("Hello, world!");
  let mut string = NtUnicodeString::try_from("Hello! ").unwrap();
  string.try_push_str("Moin!").unwrap();
  println!("{string}");

  let abc = NtUnicodeString::try_from_u16(&[b'A' as u16, b'B' as u16, b'C' as u16]).unwrap();
  let de = NtUnicodeString::try_from_u16_until_nul(&[b'D' as u16, b'E' as u16, 0]).unwrap();
  let fgh = NtUnicodeString::try_from(u16cstr!("FGH")).unwrap();
  let ijk = NtUnicodeString::try_from(u16str!("IJK")).unwrap();
  println!("{}, {}, {}, {}", abc, de, fgh, ijk);

  let string = NtUnicodeString::try_from("My String").unwrap();
  subfunction(&string);
}
