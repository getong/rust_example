use std::fs::File;
use std::io::{self, BufRead};

#[derive(Copy, Clone)]
struct Temperature {
    minimum: f32,
    maximum: f32,
}

fn get_average(temps: Vec<Temperature>) -> (f32, f32) {
    let mut min_total: f32 = 0.0;
    let mut max_total: f32 = 0.0;
    let mut count = 0;
    for t in temps {
        min_total = min_total + t.minimum;
        max_total = max_total + t.maximum;
        count = count + 1;
    }
    (min_total / count as f32, max_total / count as f32)
}

fn main() {
    let file = match File::open("temperatures.txt") {
        Err(e) => panic!("Unable to open file: {}", e),
        Ok(file) => file,
    };
    let mut daily_temps: Vec<Temperature> = Vec::new();

    let lines = io::BufReader::new(file).lines();
    for line in lines {
        let line = line.unwrap();
        let mut split_line = line.as_str().split(',');
        let left = split_line.next().unwrap();
        let right = split_line.next().unwrap();
        let today = Temperature {
            minimum: left.parse::<f32>().unwrap(),
            maximum: right.parse::<f32>().unwrap(),
        };
        daily_temps.push(today);
    }

    let avgs = get_average(daily_temps);
    println!(
        "Average daily low: {}, average daily high: {}",
        avgs.0, avgs.1
    );
}
