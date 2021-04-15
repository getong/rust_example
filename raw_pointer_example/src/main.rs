fn raw_pointers_info() {
    let mut num = 1;
    // 将引用转为裸指针
    let num_raw_point = &mut num as *mut i32;
    unsafe {
        *num_raw_point = 100;
        // Output: 100 100 0x8d8c6ff6bc
        println!("{} {} {:p}", num, *num_raw_point, &num);
    }

    let address = num_raw_point as usize;
    // 将一个 usize 对象，转化为 裸指针
    let raw = address as *mut i32;
    unsafe {
        *raw = 200;
        // Output: 200 200 0x8d8c6ff6bc 607946536636
        println!("{} {} {:p} {}", num, *num_raw_point, &num, address);
    }
}

fn main() {
    // println!("Hello, world!");
    raw_pointers_info();
}
