use std::alloc::{alloc, dealloc, Layout};

fn main() {
    unsafe {
        let layout = Layout::new::<u16>();
        let ptr = alloc(layout);

        *(ptr as *mut u16) = 42;
        assert_eq!(*(ptr as *mut u16), 42);

        dealloc(ptr, layout);
    }
}
