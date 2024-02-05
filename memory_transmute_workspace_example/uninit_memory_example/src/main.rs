use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;

struct Role {
  name: String,
  disabled: bool,
  flag: u32,
}

fn main() {
  let role = unsafe {
    let mut uninit = MaybeUninit::uninit();
    let role: *mut Role = uninit.as_mut_ptr();
    addr_of_mut!((*role).name).write("basic".to_string());
    (*role).flag = 1;
    (*role).disabled = false;
    uninit.assume_init()
  };

  println!("{} ({}, {})", role.name, role.flag, role.disabled);
}
