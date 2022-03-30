use std::os::unix::prelude::JoinHandleExt;

fn main() {
    let handle = std::thread::spawn(|| loop {
        unsafe {
            dbg!(libc::time(std::ptr::null_mut()));
            libc::sleep(1);
        }
    });
    let pthread_t = handle.into_pthread_t();
    unsafe {
        // libc::pthread_join(pthread_t, std::ptr::null_mut());
        libc::pthread_cancel(pthread_t);
        libc::pthread_cancel(pthread_t);
        libc::pthread_cancel(pthread_t);
        // libc::pthread_kill(pthread_t, libc::SIGTERM);
    }
}
