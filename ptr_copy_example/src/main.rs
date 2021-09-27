use std::ptr;

/// ＃ 安全
///
/// * `ptr` 必须与其类型正确对齐并且非零。
/// * `ptr` 必须对读取 `elts` 类型为 `T` 的连续元素有效。
/// * 调用此函数后不得使用这些元素，除非`T: Copy`。
unsafe fn from_buf_raw<T>(ptr: *const T, elts: usize) -> Vec<T> {
    let mut dst = Vec::with_capacity(elts);

    // 安全：我们的先决条件确保源对齐且有效，
    // 和 `Vec::with_capacity` 确保我们有可用的空间来编写它们。
    ptr::copy(ptr, dst.as_mut_ptr(), elts);

    // 安全：我们之前用这么多容量创建了它，
    // 而之前的 `copy` 已经初始化了这些元素。
    dst.set_len(elts);
    dst
}

fn main() {
    // println!("Hello, world!");
    let a: Vec<i32> = vec![1, 2, 3, 4];

    let dst = unsafe { from_buf_raw::<i32>(a.as_ptr(), 4) };
    println!("dst: {:?}", dst);
}
