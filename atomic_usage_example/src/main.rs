use std::sync::atomic::compiler_fence;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicUsize};

static IMPORTANT_VARIABLE: AtomicUsize = AtomicUsize::new(0);
static IS_READY: AtomicBool = AtomicBool::new(false);

fn main() {
    IMPORTANT_VARIABLE.store(42, Ordering::Relaxed);
    // prevent earlier writes from being moved beyond this point
    compiler_fence(Ordering::Release);
    IS_READY.store(true, Ordering::Relaxed);
}

// fn signal_handler() {
//     if IS_READY.load(Ordering::Relaxed) {
//         assert_eq!(IMPORTANT_VARIABLE.load(Ordering::Relaxed), 42);
//     }
// }
