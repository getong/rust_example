fn main() {
    // println!("Hello, world!");
    let x = 7;
    let y = x.to_string();
    println!("i32: {}, String: {}", x, y);

    let x = 7.7;
    let y = x.to_string();
    println!("f64: {}, String: {}", x, y);

    let x = String::from("7");
    let y = x.parse::<i32>().unwrap();
    println!("String: {}, i32: {}", x, y);

    let x = String::from("7.7");
    let y = x.parse::<f64>().unwrap();
    println!("String: {}, f64: {}", x, y);
}
