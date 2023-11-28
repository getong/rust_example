macro_rules! patterns {
    (pat: $pat:pat) => {
        println!("pat: {}", stringify!($pat));
    };
    (pat_param: $($pat:pat_param)|+) => {
        $( println!("pat_param: {}", stringify!($pat)); )+
    };
}
fn main() {
    patterns! {
        pat: 0 | 1 | 2 | 3
    }
    patterns! {
        pat_param: 0 | 1 | 2 | 3
    }
}
