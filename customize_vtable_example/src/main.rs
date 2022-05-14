use std::mem::transmute;

trait Foo {
    fn fn1(&self, x: u32);
    fn fn2(&self, x: u32)
    where
        Self: Sized;
    fn fn3(&self, x: u32);
}

#[derive(Debug)]
struct S1(u64, u64, u64);

impl Foo for S1 {
    fn fn1(&self, x: u32) {
        println!("fn1({:?}, {})", self, x);
    }
    fn fn2(&self, x: u32)
    where
        Self: Sized,
    {
        println!("fn2({:?}, {})", self, x);
    }
    fn fn3(&self, x: u32) {
        println!("fn3({:?}, {})", self, x);
    }
}

impl std::ops::Drop for S1 {
    fn drop(&mut self) {
        println!("drop({:?})", self);
    }
}

struct FooVtable {
    drop_glue: fn(&mut S1),
    size: usize,
    align: usize,
    fn1_ptr: fn(&S1, u32),
    fn2_ptr: fn(&S1, u32),
    fn3_ptr: fn(&S1, u32),
}

fn recover_s1(foo: &mut dyn Foo) -> (&mut S1, &'static FooVtable) {
    unsafe { transmute(foo) }
}

fn main() {
    let mut x = S1(3, 4, 5);
    let foo: &mut dyn Foo = &mut x;
    let (xx, vtbl) = recover_s1(foo);
    println!("vtbl.drop_glue = {:x}", vtbl.drop_glue as usize);
    println!("vtbl.size = {:x}", vtbl.size);
    println!("vtbl.align = {:x}", vtbl.align);
    println!("vtbl.fn1_ptr = {:x}", vtbl.fn1_ptr as usize);
    println!("vtbl.fn2_ptr = {:x}", vtbl.fn2_ptr as usize);
    println!("vtbl.fn3_ptr = {:x}", vtbl.fn3_ptr as usize);
    println!("S1::fn1 = {:x}", S1::fn1 as usize);
    println!("S1::fn2 = {:x}", S1::fn2 as usize);
    println!("S1::fn3 = {:x}", S1::fn3 as usize);
    (vtbl.drop_glue)(xx);
    (vtbl.fn1_ptr)(xx, 88);
    (vtbl.fn3_ptr)(xx, 188);
}
