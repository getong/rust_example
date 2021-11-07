fn origin_get_multipler(factor: &i32) -> impl Fn(i32) -> i32 + '_ {
    move |input: i32| input * factor
}

fn get_multipler(factor: i32) -> impl Fn(i32) -> i32 {
    move |input: i32| input * factor
}

fn main() {
    // println!("Hello, world!");
    let factor = 10;
    let multiplier = get_multipler(factor);

    println!("{}", multiplier(2));

    let multiplier = origin_get_multipler(&factor);

    println!("{}", multiplier(2));

    println!("factor: {}", factor);
}
