#[derive(Debug)]
struct NumRef<'a>(&'a i32);

impl<'a> NumRef<'a> {
    // no more 'a on mut self
    fn some_method(&mut self) {}

    // above line desugars to
    // fn some_method_desugared<'b>(&'b mut self) {}
}

fn main() {
    let mut num_ref = NumRef(&5);
    num_ref.some_method();
    num_ref.some_method(); // compiles
    println!("{:?}", num_ref); // compiles
}
