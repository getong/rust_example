fn box_ref<T>(b: T) -> Box<T> {
    let a = b;
    Box::new(a)
}

#[derive(Copy, Clone, Debug)]
struct Foo;

#[derive(Debug)]
struct Node {
    data: u32,
    next: Option<Box<Node>>,
}

fn main() {
    //println!("Hello, world!");
    let boxed_one = Box::new(Foo);
    println!("unboxed_one at {:p} is {:?}", boxed_one, boxed_one);
    let unboxed_one = *boxed_one;
    println!("unboxed_one at {:?} is {:?}", unboxed_one, unboxed_one);
    let boxed_two = box_ref(unboxed_one);
    println!("boxed_two at {:p} is {:?} ", boxed_two, *boxed_two);

    let node = Node {
        data: 33,
        next: None,
    };
    println!("node : {:?}", node);
}
