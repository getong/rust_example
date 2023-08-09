use std::thread;

fn main() {
    let mut v: Vec<i32> = vec![];

    let a = thread::spawn(move || {
        v.push(1);
        println!("{:?}", v);
    });

    a.join().unwrap();
    //println!("{:?}", v);
}
