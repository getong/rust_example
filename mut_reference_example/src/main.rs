// copy from [What does `&mut &[T]` mean?](https://ihatereality.space/04-what-mutref-to-slice-ref-means/)

fn mut_ref_example1() {
    let value = 1;
    let mut shared: &u32 = &value;

    println!("{r:p}: {r} (value = {v})", r = shared, v = value);
    // Prints <addr>: 1 (value = 1)

    let unique: &mut &u32 = &mut shared;
    *unique = &17;

    println!("{r:p}: {r} (value = {v})", r = shared, v = value);
    // Prints <different addr>: 17 (value = 1)
}

fn mut_ref_example2() {
    let mut slice: &[u8] = &[0, 1, 2, 3, 4];
    let unique: &mut &[u8] = &mut slice;

    // Since we want to hold unique reference,
    // we can only access the slice through it
    println!("({r:p}, {len}): {r:?}", r = *unique, len = unique.len());
    // Prints (<addr>, 5): [0, 1, 2, 3, 4]

    // Change only the length
    *unique = &unique[..4];
    println!("({r:p}, {len}): {r:?}", r = *unique, len = unique.len());
    // Prints (<addr>, 4): [0, 1, 2, 3]

    // Change both the pointer and the length
    *unique = &unique[1..];
    println!("({r:p}, {len}): {r:?}", r = *unique, len = unique.len());
    // Prints (<addr+1>, 3): [1, 2, 3]

    // Change only the pointer
    *unique = &[17, 17, 42];
    println!("({r:p}, {len}): {r:?}", r = *unique, len = unique.len());
    // Prints (<different addr>, 3): [17, 17, 42]
}

fn mut_reference_slice() {
    use std::io::Read;

    // We'll be reading *from* this slice
    let mut data: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    // And *into* this
    let mut buf = [0; 3];

    while let Ok(1..) = Read::read(&mut data, &mut buf) {
        println!("({r:p}, {len}): {r:?}", r = data, len = data.len());
        // This will print:
        // (<addr>, 7): [3, 4, 5, 6, 7, 8, 9]
        // (<addr+3>, 4): [6, 7, 8, 9]
        // (<addr+6>, 1): [9]
        // (<addr+7>, 0): []

        // In reality you'd also examine the `buf` contents here
    }
}

fn main() {
    // println!("Hello, world!");
    mut_ref_example1();

    mut_ref_example2();

    mut_reference_slice();
}
