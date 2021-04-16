mod interval;

use self::interval::Interval;

fn main() {
    let interval = Interval::from_millis(500); // half a second
    let duration = std::time::Duration::from_millis(100); // 0.1 seconds
    let mut last = interval.get_counter();
    for i in 1..51 {
        let curr = interval.get_counter();

        if curr != last {
            last = curr;
            println!("Iteration number {}, counter is {}", i, curr);
        }

        std::thread::sleep(duration);
    }
}
