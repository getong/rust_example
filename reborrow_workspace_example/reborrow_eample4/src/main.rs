struct A<'a> {
  a: &'a mut i32,
}

fn call1(A { a }: A<'_>) {
  call2(A { a });
  call3(a); // 在调用完call2之后我还想调用call3，但是A已经被move走了
}

fn call2(_: A<'_>) {}

fn call3(_: &mut i32) {}

fn main() {
  let mut n = 10;
  call1(A { a: &mut n });
}
