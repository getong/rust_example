use std::mem::transmute;

#[derive(Debug)]
struct Items(u32);

fn fat_pointers_info() {
    let mut arr = [1, 2, 3, 4];
    let slice = &mut arr[1..4];
    slice[0] = 100;
    println!("{:?}", arr); // [1, 100, 3, 4]

    let slice = &arr[1..4];
    println!("{:p} {:?}", slice, unsafe {
        transmute::<_, (usize, usize)>(slice)
    });
    // Output: 0x8a6c0ff4ac (594518471852, 3)
    //      0x8a6c0ff4ac 594518471852 这两个值是相等的。
    //      (594518471852, 3) 分别表示 具体数据的堆地址 和 长度 两个字段。
    //      注意这里是用 slice，而不是 &slice。(&slice表示这个变量本身的栈地址)

    println!("sizeof &[i32]:{}", std::mem::size_of::<&[i32]>());
    // Output: sizeof &[i32]:16
    // 因为包含了两个字段：地址 + 长度，所以其占用内存为 2 个 usize 类型大小
}

fn main() {
    let items = Items(2);
    let items_ptr = &items;
    let ref items_ref = items;
    println!("items_ptr:{:p}", items_ptr as *const Items);
    println!("items_ptr:{:p}", items_ptr);
    assert_eq!(items_ptr as *const Items, items_ref as *const Items);
    let mut a = Items(20);
    // using scope to limit the mutation of `a` within this block by b
    {
        // can take a mutable reference like this too
        let ref mut b = a; // same as: let b = &mut a;
        b.0 += 25;
    }
    println!("{:?}", items);
    println!("{:?}", a); // without the above scope

    fat_pointers_info();
}
