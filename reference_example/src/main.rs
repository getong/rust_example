fn print_type_of<T>(_: T) {
    println!("{}", std::any::type_name::<T>())
}

fn main() {
    let my_number = 15; // This is an i32
    let single_reference = &my_number; //  This is a &i32
    let double_reference = &single_reference; // This is a &&i32
    let five_references = &&&&&my_number; // This is a &&&&&i32

    println!(
        "single_reference: {:p}, double_reference: {:p}, five_references:{:p}",
        single_reference, double_reference, five_references
    );

    println!(
        "single_reference: {}, double_reference: {}, five_references:{}",
        single_reference, double_reference, five_references
    );

    print_type_of(my_number);
    print_type_of(single_reference);
    print_type_of(double_reference);
    print_type_of(five_references);
}
