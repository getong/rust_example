use rand::{self, Rng};
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::{thread, thread::JoinHandle, time::Duration};
use threadpool::ThreadPool;

fn main() {
    // first example
    let _handle: JoinHandle<()> = thread::spawn(|| {
        let delay = rand::thread_rng().gen_range(10..=2000);
        thread::sleep(Duration::from_millis(delay));
        println!("Hello from spawned thread, first");
    });
    // _ = handle.join();

    // second example
    let handle: JoinHandle<i32> = thread::spawn(|| {
        let delay = rand::thread_rng().gen_range(10..=2000);
        thread::sleep(Duration::from_millis(delay));
        println!("Hello from spawned thread, second");
        5
    });
    println!("return = {}", handle.join().unwrap());

    // third example
    let handles: Vec<JoinHandle<String>> = (0..=10)
        .map(|i| {
            let delay = rand::thread_rng().gen_range(10..=2000);
            let builder = thread::Builder::new().name(format!("Thread-{}", i));

            builder
                .spawn(move || {
                    println!("thread started = {}", thread::current().name().unwrap());
                    thread::sleep(Duration::from_millis(delay));
                    thread::current().name().unwrap().to_owned()
                })
                .unwrap()
        })
        .collect();
    for h in handles {
        let r = h.join().unwrap();
        println!("thread done = {:?}", r);
    }

    // fourth example
    let first_name_handle = thread::spawn(|| {
        thread::sleep(Duration::from_millis(2000));
        "Kevin"
    });
    let last_name_handle = thread::spawn(|| {
        thread::sleep(Duration::from_millis(2000));
        "Greene"
    });
    let name = format!(
        "{} {}",
        first_name_handle.join().unwrap(),
        last_name_handle.join().unwrap()
    );
    println!("name = {}", name);

    // fifth example
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let val: i32 = receiver.recv().unwrap();
        val + 5
    });
    sender.send(8).unwrap();
    println!("result = {}", handle.join().unwrap());

    // sixth example
    let (sender, receiver) = mpsc::channel();
    let handle = thread::spawn(move || {
        let val: i32 = receiver.recv().unwrap();
        val + 5
    });
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(2000));
        sender.send(8).unwrap();
    });
    println!("result = {}", handle.join().unwrap());

    // seventh example
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for val in receiver {
            println!("val = {}", val);
        }
    });
    for i in 0..10 {
        sender.send(i).unwrap();
        thread::sleep(Duration::from_millis(500));
    }

    // eighth example
    let (sender, receiver) = mpsc::channel();
    let receiver = Arc::new(Mutex::new(receiver));
    for i in 0..100 {
        _ = sender.send(i);
        let delay = rand::thread_rng().gen_range(1..=1000);
        thread::sleep(Duration::from_millis(delay));
    }
    for id in 0..4 {
        let receiver = Arc::clone(&receiver);
        thread::spawn(move || loop {
            let val: i32 = receiver.lock().unwrap().recv().unwrap();
            println!("val = {}, in thread-{}", val, id);
        });
    }

    // ninth example
    let receiver = spawn_thread();
    let val = receiver.recv().unwrap();
    println!("val = {}", val);

    // tenth example
    let pool = ThreadPool::new(4);
    for _ in 0..10 {
        pool.execute(|| {
            thread::sleep(Duration::from_millis(1000));
            println!("Work in thread = {}", thread::current().name().unwrap());
        });
    }
}

fn spawn_thread() -> Receiver<i32> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        sender.send(5).unwrap();
    });
    receiver
}
