use std::pin::Pin;

fn main() {
    let mut i: u8 = 9;
    let p = Pin::new(&mut i);
    assert_eq!(p.into_ref(), Pin::new(&9));
}
