fn main() {
    let stdout = 1;
    let message = "Hello, World!\n";
    let data = [wasi::Ciovec {
        buf: message.as_ptr(),
        buf_len: message.len(),
    }];
    unsafe {
        wasi::fd_write(stdout, &data).unwrap();
    }
}
