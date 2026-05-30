#![no_std]

use core::{
  alloc::{GlobalAlloc, Layout},
  cell::UnsafeCell,
  panic::PanicInfo,
  ptr::null_mut,
};

const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

struct BumpAllocator {
  heap: UnsafeCell<[u8; HEAP_SIZE]>,
  next: UnsafeCell<usize>,
}

impl BumpAllocator {
  const fn new() -> Self {
    Self {
      heap: UnsafeCell::new([0; HEAP_SIZE]),
      next: UnsafeCell::new(0),
    }
  }
}

unsafe impl Sync for BumpAllocator {}

unsafe impl GlobalAlloc for BumpAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let align_mask = layout.align().wrapping_sub(1);

    // SAFETY: Wasm component exports are entered synchronously by this demo,
    // so the bump pointer is not accessed concurrently.
    let next = unsafe { &mut *self.next.get() };
    let start = (*next + align_mask) & !align_mask;
    let end = match start.checked_add(layout.size()) {
      Some(end) if end <= HEAP_SIZE => end,
      _ => return null_mut(),
    };
    *next = end;

    // SAFETY: `end <= HEAP_SIZE`, and `start` was aligned for `layout`.
    unsafe { (*self.heap.get()).as_mut_ptr().add(start) }
  }

  unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
  loop {}
}
