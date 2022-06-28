use futures::stream::FuturesUnordered;
// use tokio::stream::StreamExt;
use rand::Rng;
use tokio::time;
use tokio_stream::StreamExt;

async fn random() -> usize {
    time::sleep(time::Duration::from_secs(1)).await;

    // 4 // chosen by fair dice roll.
    // guaranteed to be random
    let mut rng = rand::thread_rng();
    rng.gen_range(0..10)
}

#[tokio::main]
async fn main() {
    let mut numbers = FuturesUnordered::new();

    numbers.push(random()); // notice no `.await` here
    numbers.push(random());
    numbers.push(random());
    numbers.push(random());

    while let Some(number) = numbers.next().await {
        println!("{}", number);
    }
}
