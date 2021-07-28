use std::env;

extern "C" {
    // Libc global variable
    #[link_name = "environ"]
    static libc_environ: *const *const std::os::raw::c_char;
}

fn main() {
    // println!("Hello, world!");

    for (key, value) in env::vars() {
        println!("{}={}", key, value);
    }

    println!("\n\nfrom ffi\n\n");

    // like the c code
    // extern char **environ;
    // char **p;
    // for (p = environ; *p; p++) {
    // printf ("%s\n", *p);
    // }
    let mut next = unsafe { libc_environ };
    while !next.is_null() && !unsafe { *next }.is_null() {
        let env = unsafe { std::ffi::CStr::from_ptr(*next) }
            .to_str()
            .unwrap_or("<invalid>");
        println!("{}", env);
        next = unsafe { next.offset(1) };
    }
}
