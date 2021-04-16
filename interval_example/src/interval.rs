use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;

#[derive(Clone)]
pub struct Interval {
    counter: Arc<AtomicUsize>,
    still_running: Arc<AtomicBool>,
}

impl Drop for Interval {
    fn drop(&mut self) {
        println!("Interval thread shutting down");
        self.still_running.store(false, Ordering::SeqCst);
    }
}

impl Interval {
    pub fn from_millis(millis: u64) -> Interval {
        let duration = Duration::from_millis(millis);

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let still_running = Arc::new(AtomicBool::new(true));
        let still_running_clone = still_running.clone();

        spawn(move || {
            println!("Interval thread launched");
            while still_running_clone.load(Ordering::SeqCst) {
                sleep(duration);
                let old = counter_clone.fetch_add(1, Ordering::SeqCst);
                println!("Interval thread still alive, value was: {}", old);
            }
        });

        Interval {
            counter,
            still_running,
        }
    }

    pub fn get_counter(&self) -> usize {
        self.counter.load(Ordering::SeqCst)
    }
}
