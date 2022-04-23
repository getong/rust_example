use std::ptr::NonNull;

fn main() {
    let mut x = 0u32;
    let ptr = NonNull::new(&mut x as *mut _).expect("ptr is null!");
    let ref_x = unsafe { ptr.as_ref() };
    println!("ref_x: {}", ref_x);

    let mut x = 0u32;
    let mut ptr = NonNull::new(&mut x).expect("null pointer");
    let x_ref = unsafe { ptr.as_mut() };
    assert_eq!(*x_ref, 0);
    *x_ref += 2;
    assert_eq!(*x_ref, 2);

    let mut x = 0u32;
    let ptr = NonNull::new(&mut x as *mut _).expect("null pointer");
    let casted_ptr = ptr.cast::<i8>();
    let raw_ptr: *mut i8 = casted_ptr.as_ptr();
    unsafe {
        *raw_ptr = 3;
    }
    println!("*raw_ptr: {}", x);
}
