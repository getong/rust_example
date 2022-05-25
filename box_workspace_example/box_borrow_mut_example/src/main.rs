use std::borrow::BorrowMut;

trait T {}

impl T for u8 {}

fn main() {
    {
        let mut r: Box<dyn T> = Box::new(23u8);
        let _: &mut dyn T = &mut *r;
    }
    // A:
    {
        let mut r: Box<u8> = Box::new(23u8);
        let a: &mut u8 = r.borrow_mut();
        *a += 20;
        println!("r:{}", r);
    }

    // B:
    {
        let mut r: Box<dyn T> = Box::new(23u8);
        let _: &mut dyn T = r.borrow_mut();
    }
}
