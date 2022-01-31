#[derive(Debug, Default)]
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
    // we can also use this :
    dbg!(nested_struct);

    let nested_struct2 = SomeStruct {
        ..Default::default()
    };
    dbg!(nested_struct2);
}
