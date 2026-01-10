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

  async fn await_all(
    mut self,
    results: &mut Vec<(&'static str, ShutdownResult)>,
  ) -> (ShutdownTx, ShutdownRx) {
    let start = tokio::time::Instant::now();
    let mut report = tokio::time::Instant::now();
    let mut sleep = std::time::Duration::from_millis(10);

    loop {
      self.services.retain(|k, v| match v.try_recv() {
        Ok(res) => {
          results.push((*k, res));
          false
        }
        Err(oneshot::error::TryRecvError::Empty) => true,
        Err(oneshot::error::TryRecvError::Closed) => {
          results.push((*k, Err(anyhow::anyhow!("task exited without result"))));
          false
        }
      });

      if self.services.is_empty() {
        tracing::info!(
          elapsed = ?start.elapsed(),
          count = results.len(),
          "services all finished"
        );
        break;
      }

      if report.elapsed() > std::time::Duration::from_secs(5) {
        report = tokio::time::Instant::now();
        tracing::debug!(
          tasks = ?self.services.keys().collect::<Vec<_>>(),
          "tasks still running"
        );
      }

      tokio::time::sleep(sleep).await;
      sleep = std::cmp::min(
        sleep + std::time::Duration::from_millis(10),
        std::time::Duration::from_millis(100),
      );
    }

    (self.tx, self.rx)
  }
}

// copy from https://github.com/EmbarkStudios/quilkin/blob/main/src/signal.rs
