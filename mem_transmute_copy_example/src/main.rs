use std::mem;

#[repr(packed)]
struct Foo {
    bar: u8,
}

fn main() {
    // println!("Hello, world!");

    let foo_array = [10u8];

    unsafe {
        // Copy the data from 'foo_array' and treat it as a 'Foo'
        let mut foo_struct: Foo = mem::transmute_copy(&foo_array);
        assert_eq!(foo_struct.bar, 10);

        // Modify the copied data
        foo_struct.bar = 20;
        assert_eq!(foo_struct.bar, 20);
    }

    // The contents of 'foo_array' should not have changed
    assert_eq!(foo_array, [10]);
}
