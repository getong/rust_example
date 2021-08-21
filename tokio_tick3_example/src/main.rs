use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let (send, recv) = tokio::sync::mpsc::channel(100);

    let shutdown_trigger = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;

        // We get the expected behavior by commenting out this line.
        send.send(()).await.unwrap();
    });

    let done = run_with_shutdown(recv);
    done.await;
    shutdown_trigger.await.unwrap();

    Ok(())
}

async fn run_with_shutdown(mut shutdown: tokio::sync::mpsc::Receiver<()>) {
    let mut interval = time::interval(time::Duration::from_millis(10));

    loop {
        tokio::select! {
           _ = interval.tick() => println!("tick"),
           _ = shutdown.recv() => {
                println!("shutting down");
                return
           },
        }
    }
}
