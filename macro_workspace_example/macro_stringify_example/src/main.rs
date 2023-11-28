macro_rules! get_struct_name {
    ($struct_name:ident) => {
        stringify!($struct_name)
    };
}

pub struct MyStruct {
    // Fields of your struct
}

fn main() {
    // Using the macro to get the name of the struct
    let struct_name = get_struct_name!(MyStruct);
    println!("Struct name: {}", struct_name); // Output: Struct name: MyStruct
}
