use std::{collections::HashMap, time::Instant};

use deno_core::{
  ModuleSpecifier,
  anyhow::{Context, Error},
};
use deno_runtime::worker::MainWorker;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
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

/// Execution result events inspired by edge-runtime architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEvent {
  Success {
    cpu_time_ms: u64,
    wall_time_ms: u64,
  },
  Error {
    exception: String,
    cpu_time_ms: u64,
    wall_time_ms: u64,
  },
  Terminated {
    reason: String,
    cpu_time_ms: u64,
  },
}

/// Result channel for execution events
pub type ExecutionEventSender = oneshot::Sender<ExecutionEvent>;
pub type ExecutionEventReceiver = oneshot::Receiver<ExecutionEvent>;

/// TypeScript command execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
  pub command: String,
  pub args: Vec<String>,
  pub env_vars: HashMap<String, String>,
}

pub struct DenoTokioProcess<'a> {
  worker: &'a mut MainWorker,
  main_module_url: ModuleSpecifier,
  worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
  execution_event_tx: Option<ExecutionEventSender>,
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
      execution_event_tx: None,
    }
  }

  /// Create a new process with execution event channel
  pub fn new_with_events(
    worker: &'a mut MainWorker,
    main_module_url: ModuleSpecifier,
    worker_pool_tx: Option<mpsc::UnboundedSender<UserWorkerMsgs>>,
    execution_event_tx: ExecutionEventSender,
  ) -> Self {
    Self {
      worker,
      main_module_url,
      worker_pool_tx,
      execution_event_tx: Some(execution_event_tx),
    }
  }

  pub async fn run(
    mut self,
    stream: UnixStream,
    shutdown_tx: oneshot::Sender<()>,
  ) -> Result<(), Error> {
    let wall_start = Instant::now();
    let cpu_start = Self::get_cpu_time_ms();

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

    // Calculate execution metrics
    let cpu_time_ms = Self::get_cpu_time_ms() - cpu_start;
    let wall_time_ms = wall_start.elapsed().as_millis() as u64;

    // Send execution event if channel is available
    if let Some(event_tx) = self.execution_event_tx.take() {
      let event = match &run_result {
        Ok(_) => {
          info!(
            "TypeScript execution completed successfully (CPU: {}ms, Wall: {}ms)",
            cpu_time_ms, wall_time_ms
          );
          ExecutionEvent::Success {
            cpu_time_ms,
            wall_time_ms,
          }
        }
        Err(err) => {
          error!(
            "TypeScript execution failed: {err:?} (CPU: {}ms, Wall: {}ms)",
            cpu_time_ms, wall_time_ms
          );
          ExecutionEvent::Error {
            exception: format!("{err:#}"),
            cpu_time_ms,
            wall_time_ms,
          }
        }
      };

      let _ = event_tx.send(event);
    }

    if let Err(err) = &run_result {
      error!("deno tokio process encountered an error: {err:?}");
    }

    let _ = shutdown_tx.send(());

    run_result
  }

  /// Execute a TypeScript command and return the result
  /// This is a higher-level API for running TypeScript code with result capture
  pub async fn execute_command(
    mut self,
    context: ExecutionContext,
    result_tx: oneshot::Sender<Result<String, String>>,
  ) -> Result<(), Error> {
    let wall_start = Instant::now();
    let cpu_start = Self::get_cpu_time_ms();

    info!(
      "Executing TypeScript command: {} with args: {:?}",
      context.command, context.args
    );

    // Set environment variables from context
    {
      let op_state_rc = self.worker.js_runtime.op_state();
      let mut op_state = op_state_rc.borrow_mut();
      op_state.put::<EnvVars>(context.env_vars);
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
          debug!("TypeScript command event loop completed");
          run.context("event loop execution failed")?
        }
      }

      Ok::<(), Error>(())
    }
    .await;

    let cpu_time_ms = Self::get_cpu_time_ms() - cpu_start;
    let wall_time_ms = wall_start.elapsed().as_millis() as u64;

    // Send result back through the channel
    let send_result = match run_result {
      Ok(_) => {
        let success_msg = format!(
          "Command executed successfully (CPU: {}ms, Wall: {}ms)",
          cpu_time_ms, wall_time_ms
        );
        info!("{}", success_msg);
        result_tx.send(Ok(success_msg))
      }
      Err(ref err) => {
        let error_msg = format!(
          "Command failed: {err:#} (CPU: {}ms, Wall: {}ms)",
          cpu_time_ms, wall_time_ms
        );
        error!("{}", error_msg);
        result_tx.send(Err(error_msg))
      }
    };

    if send_result.is_err() {
      error!("Failed to send execution result - receiver dropped");
    }

    run_result
  }

  /// Get CPU time in milliseconds (platform-specific implementation)
  fn get_cpu_time_ms() -> u64 {
    #[cfg(target_os = "linux")]
    {
      use std::fs;
      if let Ok(stat) = fs::read_to_string("/proc/self/stat") {
        if let Some(parts) = stat.split_whitespace().nth(13) {
          if let Ok(ticks) = parts.parse::<u64>() {
            // Convert ticks to milliseconds (usually 100 ticks per second)
            return ticks * 10;
          }
        }
      }
      0
    }

    #[cfg(not(target_os = "linux"))]
    {
      // Fallback for non-Linux platforms
      0
    }
  }
}
