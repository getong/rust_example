#[allow(invalid_value)]

// copy from [A Rust curiosity: pointers to zero-sized types](https://web.archive.org/web/20160327061400/http://www.wabbo.org/blog/2014/03aug_09aug.html)
fn main() {
    let x: &() = unsafe { std::mem::transmute(0usize) };
    println!("{:?}", x); // prints '()'
}
