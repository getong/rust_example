fn main() {
    // println!("Hello, world!");
    let a = [0; 5];

    let ptr1: *const i32 = &a[1];
    let ptr2: *const i32 = &a[3];

    unsafe {
        assert_eq!(ptr2.offset_from(ptr1), 2);
        assert_eq!(ptr1.offset_from(ptr2), -2);
        assert_eq!(ptr1.offset(2), ptr2);
        assert_eq!(ptr2.offset(-2), ptr1);
    }
}
