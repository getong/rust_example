use std::collections::HashMap;

use deno_core::{
  ModuleSpecifier,
  anyhow::{Context, Error},
};
use deno_runtime::worker::MainWorker;
use log::{debug, error};
use tokio::{
  net::UnixStream,
  sync::{mpsc, oneshot},
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UserWorkerMsgs {
  Shutdown,
}

pub type EnvVars = HashMap<String, String>;

pub struct DenoTokioProcess<'a> {
  worker: &'a mut MainWorker,
  main_module_url: ModuleSpecifier,
  worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
}

impl<'a> DenoTokioProcess<'a> {
  pub fn new(
    worker: &'a mut MainWorker,
    main_module_url: ModuleSpecifier,
    worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
  ) -> Self {
    Self {
      worker,
      main_module_url,
      worker_pool_tx,
    }
  }

  pub async fn run(
    self,
    stream: UnixStream,
    shutdown_tx: oneshot::Sender<()>,
  ) -> Result<(), Error> {
    let (unix_stream_tx, unix_stream_rx) = mpsc::unbounded_channel::<UnixStream>();
    unix_stream_tx
      .send(stream)
      .map_err(|err| Error::msg(format!("failed to forward unix stream: {err:?}")))?;

    let env_vars: EnvVars = std::env::vars().collect();
    {
      let op_state_rc = self.worker.js_runtime.op_state();
      let mut op_state = op_state_rc.borrow_mut();
      op_state.put::<mpsc::UnboundedReceiver<UnixStream>>(unix_stream_rx);
      if let Some(worker_pool_tx) = self.worker_pool_tx.clone() {
        op_state.put::<mpsc::UnboundedSender<UserWorkerMsgs>>(worker_pool_tx);
      }
      op_state.put::<EnvVars>(env_vars);
    }

    let run_result = async {
      self
        .worker
        .execute_main_module(&self.main_module_url)
        .await
        .context("failed to execute main module")?;
      self
        .worker
        .dispatch_load_event()
        .context("failed to dispatch load event")?;

      tokio::select! {
        run = self.worker.run_event_loop(false) => {
          debug!("deno tokio process event loop completed");
          run.context("event loop execution failed")?
        }
      }

      Ok::<(), Error>(())
    }
    .await;

    if let Err(err) = &run_result {
      error!("deno tokio process encountered an error: {err:?}");
    }

    let _ = shutdown_tx.send(());

    run_result
  }
}
