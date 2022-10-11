macro_rules! make_local {
    () => {
        let local = 0;
    };
}

macro_rules! use_local {
    () => {
        local = 42;
    };
}

// copy from https://zjp-cn.github.io/tlborm/syntax-extensions/hygiene.html
fn main() {
    println!("Hello, world!");
    // make_local!();
    // assert_eq!(local, 0);

    // let mut local = 0;
    // use_local!();
}
