struct A<'a> {
    a: &'a mut i32,
}

fn call1(a: A) {
    call2(A { a: &mut *a.a });
    call3(a.a); // 在调用完call2之后我还想调用call3，但是A已经被move走了
}

fn call2(_: A) {}

fn call3(_: &mut i32) {}

fn main() {
    let mut n = 10;
    call1(A { a: &mut n });
}
