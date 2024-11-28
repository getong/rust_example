macro_rules! blocks {
  ($($block:block)*) => {};
}

blocks! {
    {}
    {
        let zig;
    }
    { 2 }
}

macro_rules! expressions {
  ($($expr:expr)*) => {};
}

expressions! {
    "literal"
    funcall()
    future.await
    break 'foo bar
}

macro_rules! idents {
  ($($ident:ident)*) => {};
}

idents! {
    // _ <- `_` 不是标识符，而是一种模式
    foo
    async
    O_________O
    _____O_____
}

macro_rules! items {
  ($($item:item)*) => {};
}

items! {
    struct Foo;
    enum Bar {
        Baz
    }
    impl Foo {}
    /*...*/
}

macro_rules! lifetimes {
  ($($lifetime:lifetime)*) => {};
}

lifetimes! {
    'static
    'shiv
    '_
}

macro_rules! literals {
  ($($literal:literal)*) => {};
}

literals! {
    -1
    "hello world"
    2.3
    b'b'
    true
}

macro_rules! metas {
  ($($meta:meta)*) => {};
}

metas! {
    ASimplePath
    super::man
    path = "home"
    foo(bar)
}

macro_rules! patterns {
  ($($pat:pat)*) => {};
}

patterns! {
    "literal"
    _
    0..5
    ref mut PatternsAreNice
    0 | 1 | 2 | 3
}

// macro_rules! patterns {
//    ($( $( $pat:pat_param )|+ )*) => {};
// }

// patterns! {
//    "literal"
//    _
//    0..5
//    ref mut PatternsAreNice
//    0 | 1 | 2 | 3
//}

macro_rules! paths {
  ($($path:path)*) => {};
}

paths! {
    ASimplePath
    ::A::B::C::D
    G::<eneri>::C
    FnMut(u32) -> ()
}

macro_rules! types {
  ($($type:ty)*) => {};
}

types! {
    foo::bar
    bool
    [u8]
    impl IntoIterator<Item = u32>
}

macro_rules! visibilities {
  //         ∨~~注意这个逗号，`vis` 分类符自身不会匹配到逗号
  ($($vis:vis,)*) => {};
}

visibilities! {
    , // 没有 vis 也行，因为 $vis 隐式包含 `?` 的情况
    pub,
    pub(crate),
    pub(in super),
    pub(in some_path),
}

fn main() {
  println!("hello world");
}
