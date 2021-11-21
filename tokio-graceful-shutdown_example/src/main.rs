use std::time::Duration;
use tokio_graceful_shutdown::SubsystemHandle;
use tokio_graceful_shutdown::Toplevel;

async fn subsys1(subsys: SubsystemHandle) -> Result<()> {
    log::info!("Subsystem1 started.");
    subsys.on_shutdown_requested().await;
    log::info!("Subsystem1 stopped.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    Toplevel::new()
        .start("Subsys1", subsys1)
        .catch_signals()
        .wait_for_shutdown(Duration::from_millis(1000))
        .await
}
