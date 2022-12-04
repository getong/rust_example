use std::mem::size_of;
use std::mem::transmute;

fn main() {
    println!("{:?}", size_of::<&[i32; 3]>());
    println!("{:?}", size_of::<&[i32]>());

    let v: [i32; 5] = [1, 2, 3, 4, 5];
    let p: &[i32] = &v[2..4];

    unsafe {
        let (ptr, len): (usize, isize) = transmute(p);
        println!("{} {}", ptr, len);

        let ptr = ptr as *const i32;
        for i in 0..len {
            println!("{}", *ptr.offset(i));
        }
    }

    let string: String = "abc".to_owned();
    unsafe {
        let (ptr, len, capacity): (usize, isize, usize) = transmute(string);
        println!("string: {} {} {}", ptr, len, capacity);
    }
}
