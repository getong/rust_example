use std::ffi::{c_char, c_void};

unsafe extern "C" {
  fn toml_parse_file(path: *const c_char) -> *mut c_void;
  fn toml_free_table(tbl: *const c_void);
  fn toml_print_table(tbl: *const c_void);
}

fn main() {
  unsafe {
    let table = toml_parse_file(c"Cargo.toml".as_ptr());
    if table.is_null() {
      println!("Failed to parse TOML");
    } else {
      println!("Parsed TOML successfully");
      toml_print_table(table);
    }
    toml_free_table(table);
  }
}
