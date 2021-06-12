#![feature(once_cell)] // 1.53.0-nightly (2021-04-01 d474075a8f28ae9a410e)
use std::{lazy::SyncLazy, sync::Mutex};

static ARRAY: SyncLazy<Mutex<Vec<u8>>> = SyncLazy::new(|| Mutex::new(vec![]));

fn do_a_call() {
    ARRAY.lock().unwrap().push(1);
}

fn main() {
    do_a_call();
    do_a_call();
    do_a_call();

    println!("called {}", ARRAY.lock().unwrap().len());
}
