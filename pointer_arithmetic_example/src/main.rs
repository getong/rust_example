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
}
