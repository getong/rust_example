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

fn prefix_with(prefix: &str) -> impl Fn(String) -> String + '_ {
    move |x| format!("{} {}", prefix, x)
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
}
