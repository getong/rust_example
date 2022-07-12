use std::{
    thread::{self, JoinHandle},
    time::Instant,
};

fn is_prime(n: u32) -> bool {
    (2..=n / 2).all(|i| n % i != 0)
}

fn main() {
    let now = Instant::now();
    let candidates: Vec<u32> = (2..2_000_000).collect();

    // Build a vector of thread handles, each running 1/12th of the workload
    let mut threads: Vec<JoinHandle<Vec<u32>>> = candidates
        .chunks(2_000_000 / 12)
        .into_iter()
        .map(|chunk| {
            // We have to take ownership of the chunk, otherwise the borrow
            // checked complains that we are potentially borrowing references
            // to candidates beyond the lifetime of the program.
            let my_chunk: Vec<u32> = chunk.to_owned();
            // Spawn the thread, moving our own copy of chunks into the thread.
            thread::spawn(move || {
                my_chunk
                    .iter()
                    .filter(|n| is_prime(**n))
                    .map(|n| *n)
                    .collect()
            })
        })
        .collect();

    // Combine the results
    let primes_under_2_million: Vec<u32> = threads
        .drain(0..)
        .map(|t| t.join().unwrap())
        .flatten()
        .collect();

    let elapsed = now.elapsed().as_secs_f32();
    println!(
        "Found {} primes in {:1} seconds",
        primes_under_2_million.len(),
        elapsed
    );
}
