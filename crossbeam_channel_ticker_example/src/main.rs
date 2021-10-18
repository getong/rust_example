use crossbeam_channel::tick;
use std::time::Duration;
use std::time::Instant;

fn simple_ticker() {
    let start = Instant::now();
    let ticker = tick(Duration::from_millis(100));

    for _ in 0..5 {
        let msg = ticker.recv().unwrap();
        println!("{:?} elapsed: {:?}", msg, start.elapsed());
    }
}

fn main() {
    simple_ticker();
}
