use std::mem;

fn main() {
    unsafe {
        let my_num: *mut i32 = libc::malloc(mem::size_of::<i32>() as libc::size_t) as *mut i32;
        if my_num.is_null() {
            panic!("failed to allocate memory");
        }
        libc::free(my_num as *mut libc::c_void);
    }
}
