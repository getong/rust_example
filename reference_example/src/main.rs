fn print_type_of<T>(_: T) {
    println!("{}", std::any::type_name::<T>())
}

struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug)]
pub struct MyStruct<'a, 'b>
where
    'a: 'b,
{
    pub data1: &'a str,
    pub data2: &'b str,
}

#[derive(Debug)]
pub struct MyStruct2<'a, 'b>
where
    'a: 'b,
{
    pub data1: &'a String,
    pub data2: &'b String,
}

#[derive(Debug)]
pub struct MyStruct3<'a, 'b, T>
where
    'a: 'b,
    T: 'a,
{
    pub data1: &'a T,
    pub data2: &'b T,
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

    // reference and mutable reference
    let mut value = 42;

    let shared_ref = &value; // Shared reference
    println!("Shared reference: {}", shared_ref);

    let mutable_ref = &mut value; // Mutable reference
    *mutable_ref += 10; // Modify the value through the mutable reference
    println!("Modified value: {}", value);

    // struct lifetime
    let data1 = "Hello";
    {
        let data2 = "World";

        let my_struct = MyStruct {
            data1: &data2,
            data2: &data1,
        };
        println!("my_struct {:?}", my_struct);
    }

    // struct lifetime
    let data1 = "Hello";
    let data2 = "World";

    let my_struct = MyStruct {
        data1: &data2,
        data2: &data1,
    };
    println!("my_struct {:?}", my_struct);

    // struct lifetime
    let data1 = "Hello".to_string();
    let data2 = "World".to_string();

    let my_struct = MyStruct {
        data1: &data2,
        data2: &data1,
    };
    println!("my_struct {:?}", my_struct);

    // struct lifetime
    let data1: String = "Hello".to_string();
    {
        let data2: String = "World".to_string();

        let my_struct = MyStruct2 {
            data1: &data2,
            data2: &data1,
        };
        println!("my_struct {:?}", my_struct);
    }

    let data1: String = "Hello".to_string();
    {
        let data2: String = "World".to_string();

        let my_struct = MyStruct3 {
            data1: &data1,
            data2: &data2,
        };
        println!("my_struct {:?}", my_struct);
    }
}
