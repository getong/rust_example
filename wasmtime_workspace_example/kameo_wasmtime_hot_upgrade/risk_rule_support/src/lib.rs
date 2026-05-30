#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(target_arch = "wasm32")]
use core::{
  alloc::{GlobalAlloc, Layout},
  cell::UnsafeCell,
  panic::PanicInfo,
  ptr::null_mut,
};

#[cfg(target_arch = "wasm32")]
const HEAP_SIZE: usize = 64 * 1024;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

#[cfg(target_arch = "wasm32")]
struct BumpAllocator {
  heap: UnsafeCell<[u8; HEAP_SIZE]>,
  next: UnsafeCell<usize>,
}

#[cfg(target_arch = "wasm32")]
impl BumpAllocator {
  const fn new() -> Self {
    Self {
      heap: UnsafeCell::new([0; HEAP_SIZE]),
      next: UnsafeCell::new(0),
    }
  }
}

#[cfg(target_arch = "wasm32")]
unsafe impl Sync for BumpAllocator {}

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
  loop {}
}
