use std::sync::{Arc, Mutex};

use std::thread;
use std::time::Duration;

struct JobStatus {
    jobs_completed: u32,
}

#[derive(Clone)]
struct ConcurrentStack<T> {
    inner: Arc<Mutex<Vec<T>>>,
}

impl<T> ConcurrentStack<T> {
    pub fn new() -> Self {
        ConcurrentStack {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push(&self, data: T) {
        let mut inner = self.inner.lock().unwrap();
        // (*inner).push(data);
        inner.push(data);
    }

    pub fn pop(&self) -> Option<T> {
        let mut inner = self.inner.lock().unwrap();
        // (*inner).pop()
        inner.pop()
    }
}

fn main() {
    let status = Arc::new(Mutex::new(JobStatus { jobs_completed: 0 }));
    let status_shared = status.clone();
    thread::spawn(move || {
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(250));
            status_shared.lock().unwrap().jobs_completed += 1;
        }
    });
    while status.lock().unwrap().jobs_completed < 10 {
        println!("waiting... ");
        thread::sleep(Duration::from_millis(500));
    }

    let con_stack: ConcurrentStack<i32> = ConcurrentStack::new();
    con_stack.push(1);
    if let Some(num) = con_stack.pop() {
        println!("num : {:?}", num);
    }
}
