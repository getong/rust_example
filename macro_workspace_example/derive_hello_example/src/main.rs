use derive_hello_example::Hello;

#[derive(Hello)]
enum Pet {
    Cat,
}

fn main() {
    // previous code
    let p = Pet::Cat;
    p.hello_world();
}
