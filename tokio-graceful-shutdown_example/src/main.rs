use anyhow::Result;
use env_logger::{Builder, Env};
use tokio::time::{sleep, Duration};
use tokio_graceful_shutdown::{SubsystemHandle, Toplevel};

async fn countdown() {
    for i in (1..=5).rev() {
        log::info!("Shutting down in: {}", i);
        sleep(Duration::from_millis(1000)).await;
    }
}

async fn countdown_subsystem(subsys: SubsystemHandle) -> Result<()> {
    tokio::select! {
        _ = subsys.on_shutdown_requested() => {
            log::info!("Countdown cancelled.");
        },
        _ = countdown() => {
            subsys.request_shutdown();
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Create toplevel
    Toplevel::new()
        .start("Countdown", countdown_subsystem)
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
