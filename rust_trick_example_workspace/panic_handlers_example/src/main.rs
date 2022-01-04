use std::panic::{set_hook, take_hook};

fn main() {
    let prev_hook = take_hook();

    set_hook(Box::new(move |panic| {
        println!("custom logging logic {}", panic);

        prev_hook(panic);
    }));

    let prev_hook = take_hook();

    set_hook(Box::new(move |panic| {
        println!("custom error reporting logic {}", panic);

        prev_hook(panic);
    }));

    panic!("test")

    // Output:
    // custom error reporting logic panicked at 'test', src/main.rs:20:5
    // custom logging logic panicked at 'test', src/main.rs:20:5
}
