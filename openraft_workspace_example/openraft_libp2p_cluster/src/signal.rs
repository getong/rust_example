use std::collections::BTreeMap;

use tokio::sync::{oneshot, watch};

/// Receiver for a shutdown event.
pub type ShutdownRx = watch::Receiver<()>;
pub type ShutdownTx = watch::Sender<()>;
pub type ShutdownResult = anyhow::Result<()>;

/// Creates a new handler for shutdown signal (e.g. SIGTERM, SIGINT), and
/// returns a handler that will broadcast shutdown to subscribers.
pub fn spawn_handler() -> ShutdownHandler {
  let (tx, rx) = channel();

  #[cfg(unix)]
  let mut sig_term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    .expect("install SIGTERM handler");

  let shutdown_tx = tx.clone();
  tokio::spawn(async move {
    #[cfg(unix)]
    let sig_term = sig_term.recv();
    #[cfg(not(unix))]
    let sig_term = std::future::pending::<()>();

    let signal = tokio::select! {
      _ = tokio::signal::ctrl_c() => "SIGINT",
      _ = sig_term => "SIGTERM",
    };

    tracing::info!(%signal, "shutting down from signal");
    // Don't unwrap so we can still run any subsequent shutdown tasks.
    let _ = shutdown_tx.send(());
  });

  ShutdownHandler::new(tx, rx)
}

pub fn channel() -> (ShutdownTx, ShutdownRx) {
  watch::channel(())
}

pub struct ShutdownHandler {
  tx: ShutdownTx,
  rx: ShutdownRx,
  services: BTreeMap<&'static str, oneshot::Receiver<ShutdownResult>>,
}

impl ShutdownHandler {
  pub fn new(tx: ShutdownTx, rx: ShutdownRx) -> Self {
    Self {
      tx,
      rx,
      services: BTreeMap::new(),
    }
  }

  pub fn push(&mut self, svc: &'static str) -> oneshot::Sender<ShutdownResult> {
    let (tx, rx) = oneshot::channel();
    if self.services.insert(svc, rx).is_some() {
      panic!("service '{svc}' already registered");
    }
    tx
  }

  pub fn shutdown_rx(&self) -> ShutdownRx {
    self.rx.clone()
  }

  pub fn shutdown_tx(&self) -> ShutdownTx {
    self.tx.clone()
  }

  pub async fn wait_signal(
    mut self,
  ) -> (ShutdownTx, ShutdownRx, Vec<(&'static str, ShutdownResult)>) {
    let _ = self.rx.changed().await;
    let mut results = Vec::with_capacity(self.services.len());
    let (t, r) = self.await_all(&mut results).await;
    (t, r, results)
  }

  pub async fn shutdown(self) -> (ShutdownTx, ShutdownRx, Vec<(&'static str, ShutdownResult)>) {
    let _ = self.tx.send(());
    let mut results = Vec::with_capacity(self.services.len());
    let (t, r) = self.await_all(&mut results).await;
    (t, r, results)
  }

  pub async fn await_any_then_shutdown(
    mut self,
  ) -> (ShutdownTx, ShutdownRx, Vec<(&'static str, ShutdownResult)>) {
    let (which, res) = {
      let mut completions = std::pin::pin!(&mut self.services);
      let mut srx = std::pin::pin!(self.rx.changed());
      std::future::poll_fn(move |cx| {
        use std::task::Poll;

        if srx.as_mut().poll(cx).is_ready() {
          return Poll::Ready(("", Ok(())));
        }

        for (key, value) in completions.as_mut().iter_mut() {
          if let Poll::Ready(res) = std::pin::pin!(value).as_mut().poll(cx) {
            return Poll::Ready((key, res.unwrap_or(Ok(()))));
          }
        }

        Poll::Pending
      })
      .await
    };

    let mut results = Vec::with_capacity(self.services.len());

    // If a task exited first, signal shutdown so others know to stop.
    if !which.is_empty() {
      let _ = self.tx.send(());
      results.push((which, res));
    }

    let (t, r) = self.await_all(&mut results).await;
    (t, r, results)
  }

  /// Await every registered service's completion oneshot. Event-driven:
  /// each receiver is awaited directly (no polling loop), so completions are
  /// observed immediately instead of after up to 100ms of backoff sleep. A
  /// 5s ticker still logs which services are outstanding.
  async fn await_all(
    self,
    results: &mut Vec<(&'static str, ShutdownResult)>,
  ) -> (ShutdownTx, ShutdownRx) {
    use futures::stream::{FuturesUnordered, StreamExt};

    let Self { tx, rx, services } = self;
    let start = tokio::time::Instant::now();

    let mut remaining: std::collections::BTreeSet<&'static str> =
      services.keys().copied().collect();
    let mut waits: FuturesUnordered<_> = services
      .into_iter()
      .map(|(name, done_rx)| async move {
        let res = done_rx
          .await
          .unwrap_or_else(|_| Err(anyhow::anyhow!("task exited without result")));
        (name, res)
      })
      .collect();

    let mut report_tick = tokio::time::interval(std::time::Duration::from_secs(5));
    report_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    report_tick.tick().await;

    while !waits.is_empty() {
      tokio::select! {
        Some((name, res)) = waits.next() => {
          remaining.remove(name);
          results.push((name, res));
        }
        _ = report_tick.tick() => {
          tracing::debug!(tasks = ?remaining, "tasks still running");
        }
      }
    }

    tracing::info!(
      elapsed = ?start.elapsed(),
      count = results.len(),
      "services all finished"
    );

    (tx, rx)
  }
}

// copy from https://github.com/EmbarkStudios/quilkin/blob/main/src/signal.rs
#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn shutdown_collects_all_service_results() {
    let (tx, rx) = channel();
    let mut handler = ShutdownHandler::new(tx, rx);

    let a_done = handler.push("service-a");
    let b_done = handler.push("service-b");
    let mut a_shutdown = handler.shutdown_rx();
    let mut b_shutdown = handler.shutdown_rx();

    tokio::spawn(async move {
      let _ = a_shutdown.changed().await;
      let _ = a_done.send(Ok(()));
    });
    tokio::spawn(async move {
      let _ = b_shutdown.changed().await;
      // Dropping without sending simulates a task that died mid-shutdown.
      drop(b_done);
    });

    let (_tx, _rx, mut results) = handler.shutdown().await;
    results.sort_by_key(|(name, _)| *name);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "service-a");
    assert!(results[0].1.is_ok());
    assert_eq!(results[1].0, "service-b");
    assert!(
      results[1].1.is_err(),
      "dropped oneshot must surface as error"
    );
  }
}
