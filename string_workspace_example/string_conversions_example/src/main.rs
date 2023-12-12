use hex::ToHex;
// use libc::c_char;
use primitive_types::H160;
use primitive_types::U256;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use std::str::FromStr;

fn main() {
  // println!("Hello, world!");

  // From String to CString
  let s = String::from("random");
  let cs = CString::new(s.clone()).unwrap();

  // From CString to C char pointer

  // let cs: libc::c_char = 8;
  // let p: *mut libc::c_char = cs.clone().into_raw();

  // From C char pointer to CStr

  extern "C" {
    fn my_string() -> *const c_char;
  }

  unsafe {
    let slice = CStr::from_ptr(my_string());
    println!(
      "string buffer size without nul terminator: {}",
      slice.to_bytes().len()
    );
  }

  // From C char pointer to String

  // let cp: *mut c_char = cs.clone().into_raw();
  // let cs: String = CStr::from_ptr(cp).to_owned().into_string().unwrap();

  // From String to Vec<u8>
  let s: String = "abc".to_owned();
  let v: Vec<u8> = s.as_bytes().into();

  // Hex encoded string to Vec<u8>

  let s_hex = "Hello world!".encode_hex::<String>();
  let data = hex::decode(s_hex).unwrap();

  // From Vec<u8> to String
  let msg_data: Vec<u8> = vec![1, 2, 3];
  let msg = String::from_utf8_lossy(&msg_data);

  // Integer types to hex
  let i = 1024;
  println!(
    "{} as hex without prefix:                             {:x}",
    i, i
  );
  // 1024 as hex without prefix:                             400
  println!(
    "{} as hex with 0x prefix:                             {:#x}",
    i, i
  );
  // 1024 as hex with 0x prefix:                             0x400
  println!(
    "{} as hex with left padding zeros and without prefix: {:08x}",
    i, i
  );
  // 1024 as hex with left padding zeros and without prefix: 00000400
  println!(
    "{} as hex with left padding zeros and with 0x prefix: {:#08x}",
    i, i
  );
  // 1024 as hex with left padding zeros and with 0x prefix: 0x000400

  // U256 to hex string

  let amount = U256::from_dec_str("9999").unwrap();
  // If you don't want the `0x` prefix:
  let amount_as_hex = format!("{:064x}", amount);
  // If you want the `0x` prefix:
  let amount_as_hex = format!("{:#066x}", amount);

  // Address/H160 and hex string conversion

  let addr = H160::from_str("0xf000000000000000000000000000000000000000").unwrap();
  // ToHex is already done the encoding part for us, we only need to pad zeros on the left side
  println!("{:0>64}", addr.encode_hex::<String>());
}
