// copy from [Dealing with long-lived child processes in Rust: Part 1](https://www.nikbrendler.com/rust-process-communication/)

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use std::thread;
use std::thread::sleep;
use std::time::Duration;

fn start_process(sender: Sender<String>, receiver: Receiver<String>) {
    let child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start process");

    println!("Started process: {}", child.id());

    thread::spawn(move || {
        let mut f = BufReader::new(child.stdout.unwrap());
        let mut stdin = child.stdin.unwrap();
        for line in receiver {
            stdin.write_all(line.as_bytes()).unwrap();
            let mut buf = String::new();
            match f.read_line(&mut buf) {
                Ok(_) => {
                    sender.send(buf).unwrap();
                    continue;
                }
                Err(e) => {
                    println!("an error!: {:?}", e);
                    break;
                }
            }
        }
    });
}

fn start_command_thread(mutex: Mutex<Sender<String>>) {
    thread::spawn(move || {
        let sender = mutex.lock().unwrap();
        sleep(Duration::from_secs(3));
        sender
            .send(String::from("Command from the thread\n"))
            .unwrap();
    });
}

fn main() {
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();

    start_process(tx1, rx2);

    tx2.send(String::from("Command 1\n")).unwrap();
    start_command_thread(Mutex::new(tx2));

    for line in rx1 {
        println!("Got this back: {}", line);
    }
}
