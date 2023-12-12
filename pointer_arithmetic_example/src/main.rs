#![feature(pointer_byte_offsets)] // at top of file
fn main() {
  let array: [i32; 3] = [10, 20, 30];
  let ptr: *const i32 = array.as_ptr();
  // let new_ptr = unsafe { ptr.add(1) };
  let new_ptr = unsafe { ptr.add(3) };
  let value = unsafe { *new_ptr };
  println!("Value: {}", value);

  let new_ptr = unsafe { ptr.add(3) };
  let value = unsafe { *new_ptr };
  println!("Value: {}", value);

  // cargo-run segmentation fault here
  let base_addr = 0x0112A160 as *mut u32;
  println!("data: {:?}", unsafe { *base_addr });

  let new_address = unsafe { base_addr.byte_offset(0xF8) };
  println!("data: {:?}", unsafe { *new_address });
  let new_address = unsafe { base_addr.offset(0xF8) };
  println!("data: {:?}", unsafe { *new_address });
}
