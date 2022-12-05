#[derive(Debug)]
struct R<'a>(&'a i32);

unsafe fn extend_lifetime<'b>(r: R<'b>) -> R<'static> {
    std::mem::transmute::<R<'b>, R<'static>>(r)
}

unsafe fn shorten_invariant_lifetime<'b, 'c>(r: &'b mut R<'static>) -> &'b mut R<'c> {
    std::mem::transmute::<&'b mut R<'static>, &'b mut R<'c>>(r)
}

fn main() {
    // println!("Hello, world!");
    let r1 = R(&32);
    let r2 = unsafe { extend_lifetime(r1) };
    println!("r2:{:?}", r2);

    let mut r3 = R(&32);
    let r4 = unsafe { shorten_invariant_lifetime(&mut r3) };
    println!("r4:{:?}", r4);
}
