#[derive(Debug)]
struct SomeStruct {
    inner: Option<Box<SomeStruct>>,
}

fn main() {
    // println!("Hello, world!");
    let nested_struct = SomeStruct {
        inner: Some(Box::new(SomeStruct {
            inner: Some(Box::new(SomeStruct { inner: None })),
        })),
    };

    println!("{nested_struct:#?}");
}
