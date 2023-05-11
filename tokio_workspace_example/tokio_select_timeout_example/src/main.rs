use std::io::stdin;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

async fn create_timeout(duration: Duration) {
    tokio::spawn(sleep(duration)).await.unwrap();
}

async fn get_user_input() -> String {
    let (s, mut r) = mpsc::unbounded_channel();
    thread::spawn(move || {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        s.send(input).unwrap();
    });
    r.recv().await.unwrap()
}

#[tokio::main]
async fn main() {
    let timeout = create_timeout(Duration::from_secs(2));
    let user_input = get_user_input();

    tokio::select! {
        input = user_input => println!("User entered: {:?}", input),
        _ =  timeout => println!("Timed out"),
    }
}