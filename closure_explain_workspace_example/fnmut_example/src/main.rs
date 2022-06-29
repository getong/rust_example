/*
struct MyClosure {
    i: &mut i32,
}

impl FnMut for Myclosure {
    fn call_mut(&mut self) {
        *self.i += 1;
    }
}

*/

fn main() {
    // println!("Hello, world!");

    let mut i: i32 = 0;
    let mut f = || {
        i += 1;
    };

    f();
    f();

    println!("{}", i);
}
