#[repr(C)]
union Metric {
    rounded: u32,
    precise: f32,
}

fn main() {
    // println!("Hello, world!");
    let mut a = Metric { rounded: 323 };
    unsafe {
        println!("{}", a.rounded);
    }

    unsafe {
        println!("{}", a.precise);
    }

    a.precise = 33.3;
    unsafe {
        println!("{}", a.precise);
    }

    unsafe {
        println!("{}", a.rounded);
    }
}
