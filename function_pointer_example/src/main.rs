fn hello() {
    println!("hello function pointer!");
}

fn print_type_of<T>(_: T) {
    println!("{}", std::any::type_name::<T>())
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
}
