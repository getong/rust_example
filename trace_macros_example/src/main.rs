#![feature(trace_macros)]

macro_rules! my_vec { //#1
    () => [ //#2
        Vec::new()
    ]; //#3

    (make an empty vec) => ( //#4
        Vec::new()
    ); //#5

    {$x:expr} => {
        {
            let mut v = Vec::new();
            v.push($x);
            v
        }
    }; //#6

    [$($x:expr),+,] => (
        {
            let mut v = Vec::new();
            $(
                v.push($x);
            )+
                v
        }
    );

    [$($x:expr),+] => (
        {
            let mut v = Vec::new();
            $(
                v.push($x);
            )+
                v
        }
    );

}

fn add_one(n: i32) -> i32 {
    n + 1
}
fn stringify(n: i32) -> String {
    n.to_string()
}

fn prefix_with(prefix: &str) -> impl Fn(String) -> String + '_ {
    move |x| format!("{} {}", prefix, x)
}

fn compose_two<FIRST, SECOND, THIRD, F, G>(f1: F, f2: G) -> impl Fn(FIRST) -> THIRD
where
    F: Fn(FIRST) -> SECOND,
    G: Fn(SECOND) -> THIRD,
{
    move |x| f2(f1(x))
}

fn main() {
    trace_macros!(true);
    println!("Hello, Rust!");

    let a = my_vec![1, 2, 3,];
    println!("a: {:?}", a);

    let b = my_vec![1, 2, 3,];
    println!("b: {:?}", b);
    trace_macros!(false);

    let c = prefix_with("hello");
    println!("{:?}", c("world".to_string()));

    let two_composed_function =
        compose_two(compose_two(add_one, stringify), prefix_with("Result: "));

    let d = two_composed_function(3);
    println!("d:{:?}", d);
}
