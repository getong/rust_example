use backtrace::Backtrace;

struct S {
    a: u32,
    b: u64,
}

impl S {
    fn new() -> Self {
        println!("(!) allocating at:\n{:?}", Backtrace::new());
        Self { a: 12, b: 24 }
    }
}

impl Drop for S {
    fn drop(&mut self) {
        println!("(!) freeing at:\n{:?}", Backtrace::new());
    }
}

fn main() {
    let s = S::new();
    dbg!(s.a, s.b);
}
