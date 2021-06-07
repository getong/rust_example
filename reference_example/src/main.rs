fn print_type_of<T>(_: T) {
    println!("{}", std::any::type_name::<T>())
}

struct Point {
    x: i32,
    y: i32,
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

    let point = Point { x: 1000, y: 729 };
    let r: &Point = &point;
    let rr: &&Point = &r;
    let rrr: &&&Point = &rr;
    assert_eq!(rrr.y, 729);
    assert_eq!(rrr.x, 1000);
}
