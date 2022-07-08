#[repr(transparent)]
#[derive(Copy, Clone)]
struct CallbackFn(fn() -> i32);

fn foo() -> i32 {
    42
}

fn main() {
    // 装有函数指针的结构体
    let fn_a = CallbackFn(foo);

    println!("a: {}", fn_a.0());

    // 装有函数指针的 Box
    let fn_b: Box<fn() -> i32> = Box::new(foo);
    println!("a: {}", fn_b());

    // 将 Box 转换为裸指针
    let fn_c: *const fn() -> i32 = fn_b.as_ref() as *const fn() -> i32;
    unsafe {
        println!("a: {}", (*fn_c)());
    }
}
