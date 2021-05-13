fn hello() {
    println!("hello function pointer!");
}

fn print_type_of<T>(_: T) {
    println!("{}", std::any::type_name::<T>())
}

fn one(n: i32) -> i32 {
    n + 1
}

fn two(n: i32) -> i32 {
    n + 2
}

fn three(n: i32) -> i32 {
    n + 3
}

fn main() {
    // println!("Hello, world!");
    let fn_ptr: fn() = hello;
    println!("{:p}", fn_ptr);

    let other_fn = hello;
    // println!("{:p}", other_fn);

    fn_ptr();
    other_fn();

    // output fn()
    print_type_of(fn_ptr);
    // output function_pointer_example::hello
    print_type_of(other_fn);

    let f1: fn(i32) -> i32 = one;
    let f2: fn(i32) -> i32 = two;
    let f3: fn(i32) -> i32 = three;

    let funcs = [f1, f2, f3];

    for f in &funcs {
        println!("{:?}", f(1));
    }
}
