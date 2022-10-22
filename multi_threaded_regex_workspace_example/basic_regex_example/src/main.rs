use lazy_static::lazy_static;
use regex::Regex;
use std::time::Instant;

lazy_static! {
    static ref IS_INTEGER: Regex = Regex::new("^[0-9]+$").unwrap();
}

fn main() {
    let strings: Vec<&str> = ["foo", "bar", "1234", "1234foo", ""]
        .into_iter()
        .cycle()
        .take(100_000_000)
        .collect();

    let start = Instant::now();
    let n_ints = strings.iter().filter(|s| IS_INTEGER.is_match(s)).count();
    let elapsed = start.elapsed().as_secs_f32();
    println!("{} {}s", n_ints, elapsed);
}
