use std::cell::UnsafeCell;

fn main() {
    // println!("Hello, world!");

    let mut x: UnsafeCell<i32> = 42.into();

    // Get a compile-time-checked unique reference to `x`.
    let p_unique: &mut UnsafeCell<i32> = &mut x;
    // With an exclusive reference, we can mutate the contents for free.
    *p_unique.get_mut() = 0;
    // Or, equivalently:
    //x = UnsafeCell::new(0);

    // When we own the value, we can extract the contents for free.
    let contents: i32 = x.into_inner();
    assert_eq!(contents, 0);
}
