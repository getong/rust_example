use tokio::time;

async fn print() {
    let mut interval = time::interval(time::Duration::from_secs(1));
    loop {
        interval.tick().await;
        println!("2333");
    }
}

#[tokio::main]
async fn main() {
    tokio::spawn(print());
    std::thread::sleep(std::time::Duration::from_secs(3));
}
