fn create_closure() -> impl Fn() {
    let msg = String::from("hello");
    let v: Vec<i32> = vec![1, 2];

    // please note that, move keyword
    move || {
        println!("{}", msg);
        println!("{:?}", v);
    }
}

/*
struct MyClosure {
    msg: String,
    v: Vec<i32>,
}

impl Fn for Myclosure {
    fn call(&self) {
        println!("{}", msg);
        println!("{:?}", v);
    }
}

*/

fn create_closure2() {
    let msg = String::from("hello");

    let my_print = || {
        println!("{}", msg);
    };

    my_print();
    my_print();
}

/*
struct MyClosure2 {
    i: &String,
}

impl Fn for Myclosure2 {
    fn call(&self) {
        println!("{}", self.msg);
    }
}

*/

fn main() {
    // println!("Hello, world!");

    let a = create_closure();
    a();
    a();

    create_closure2();
    create_closure2();
}
