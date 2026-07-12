//! Generic multi-kind task execution, octopii-style: like octopii's
//! `StateMachineTrait` + `Arc<dyn ...>` plug-in, the worker pipeline knows
//! nothing about concrete tasks. A payload is self-describing JSON tagged
//! with `kind` (`{"kind":"email","to":"a@b"}`); the [`TaskHandlerRegistry`]
//! routes it to the [`TaskHandler`] registered for that kind. Payloads
//! written before the tag existed (bare `{"to":..}`) still run as
//! [`LEGACY_KIND`].
//!
//! Adding a task type = implement [`TypedTaskHandler`] (payload decoding for
//! free) or the object-safe [`TaskHandler`], then one `register()` call —
//! claim/execute/ack, retry/backoff, timeouts and concurrency are shared.
//! Kinds are NOT mutually exclusive: dispatch is per task, so one worker
//! interleaves any mix of kinds up to its execution-permit cap.

use std::{collections::HashMap, fmt::Write as _, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use tokio::sync::Mutex;

use crate::{
  groups,
  network::transport::Libp2pNetworkFactory,
  tasks::{
    TaskRecord,
    rpc::ControlNodes,
    worker::{TASK_EXECUTION_TIMEOUT, submit_reply},
  },
  types_kv::Request as StateCommand,
};

/// Kind assumed for payloads without a `kind` tag (records enqueued before
/// multi-kind tasks existed).
pub const LEGACY_KIND: &str = "email";

/// Cluster plumbing available to handlers that talk back to the control
/// plane (raft writes / read RPCs).
pub struct TaskClusterAccess<'a> {
  pub network: &'a Libp2pNetworkFactory,
  pub control_nodes: &'a Mutex<ControlNodes>,
}

/// Execution context handed to every handler. Cluster access is optional so
/// pure-computation handlers stay unit-testable without a libp2p swarm.
pub struct TaskCtx<'a> {
  cluster: Option<TaskClusterAccess<'a>>,
}

impl<'a> TaskCtx<'a> {
  pub fn with_cluster(
    network: &'a Libp2pNetworkFactory,
    control_nodes: &'a Mutex<ControlNodes>,
  ) -> Self {
    Self {
      cluster: Some(TaskClusterAccess {
        network,
        control_nodes,
      }),
    }
  }

  /// Context without cluster plumbing (tests, standalone tools).
  pub fn detached() -> TaskCtx<'static> {
    TaskCtx { cluster: None }
  }

  pub fn cluster(&self) -> Result<&TaskClusterAccess<'a>, String> {
    self
      .cluster
      .as_ref()
      .ok_or_else(|| "handler requires cluster access, but this context has none".to_string())
  }
}

/// Object-safe task handler (the octopii `StateMachineTrait` analogue).
/// `Ok(result)` is stored on the record as opaque JSON via `TaskDone`;
/// `Err` goes through the shared retry/backoff path.
#[async_trait::async_trait]
pub trait TaskHandler: Send + Sync {
  /// The payload `kind` tag this handler owns.
  fn kind(&self) -> &'static str;

  async fn execute(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    raw_payload: &str,
  ) -> Result<Option<String>, String>;
}

/// Typed sugar over [`TaskHandler`]: declare the payload type and get JSON
/// decoding (with a uniform error message) for free.
#[async_trait::async_trait]
pub trait TypedTaskHandler: Send + Sync {
  const KIND: &'static str;
  type Payload: DeserializeOwned + Send;

  async fn run(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    payload: Self::Payload,
  ) -> Result<Option<String>, String>;
}

#[async_trait::async_trait]
impl<H: TypedTaskHandler> TaskHandler for H {
  fn kind(&self) -> &'static str {
    Self::KIND
  }

  async fn execute(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    raw_payload: &str,
  ) -> Result<Option<String>, String> {
    let payload: H::Payload = sonic_rs::from_str(raw_payload)
      .map_err(|err| format!("decode {} payload: {err}", Self::KIND))?;
    self.run(ctx, record, payload).await
  }
}

/// Extract the `kind` tag from a payload. `Ok(None)` means the payload is a
/// JSON object without a tag (legacy).
pub fn payload_kind(payload: &str) -> Result<Option<String>, String> {
  #[derive(Deserialize)]
  struct Tag {
    #[serde(default)]
    kind: Option<String>,
  }
  let tag: Tag = sonic_rs::from_str(payload)
    .map_err(|err| format!("task payload is not a JSON object: {err}"))?;
  Ok(tag.kind)
}

/// `kind` → handler routing table. Workers start from [`builtin()`];
/// embedders can register additional handlers before starting the worker.
///
/// [`builtin()`]: TaskHandlerRegistry::builtin
#[derive(Default)]
pub struct TaskHandlerRegistry {
  handlers: HashMap<&'static str, Arc<dyn TaskHandler>>,
}

impl TaskHandlerRegistry {
  pub fn new() -> Self {
    Self::default()
  }

  /// Every handler shipped with this crate.
  pub fn builtin() -> Self {
    let mut registry = Self::new();
    registry
      .register(Arc::new(EmailHandler))
      .register(Arc::new(WebhookHandler))
      .register(Arc::new(DigestHandler))
      .register(Arc::new(KvSetHandler))
      .register(Arc::new(SleepHandler));
    registry
  }

  /// Register a handler under its `kind()`. Last registration wins, so an
  /// embedder can override a builtin.
  pub fn register(&mut self, handler: Arc<dyn TaskHandler>) -> &mut Self {
    let kind = handler.kind();
    if self.handlers.insert(kind, handler).is_some() {
      tracing::warn!(kind, "task handler kind re-registered; overriding");
    }
    self
  }

  pub fn kinds(&self) -> Vec<&'static str> {
    let mut kinds: Vec<&'static str> = self.handlers.keys().copied().collect();
    kinds.sort_unstable();
    kinds
  }

  pub fn get(&self, kind: &str) -> Option<&Arc<dyn TaskHandler>> {
    self.handlers.get(kind)
  }

  /// Decode the payload's kind tag and dispatch to its handler.
  pub async fn execute(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
  ) -> Result<Option<String>, String> {
    let kind = payload_kind(&record.payload)?.unwrap_or_else(|| LEGACY_KIND.to_string());
    let handler = self.get(&kind).ok_or_else(|| {
      format!(
        "no handler registered for task kind {kind:?} (registered: {:?})",
        self.kinds()
      )
    })?;
    handler.execute(ctx, record, &record.payload).await
  }
}

// ---------------------------------------------------------------------------
// Builtin handlers
// ---------------------------------------------------------------------------

/// The original demo task: pretend-send an email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
  pub to: String,
}

pub struct EmailHandler;

#[async_trait::async_trait]
impl TypedTaskHandler for EmailHandler {
  const KIND: &'static str = "email";
  type Payload = Email;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    email: Email,
  ) -> Result<Option<String>, String> {
    // Testable failure semantics for drills: recipients starting with "fail"
    // simulate a handler error (exercises retry/backoff and permanent
    // failure), "slow" simulates a hang (exercises the execution timeout).
    if email.to.starts_with("fail") {
      return Err(format!("simulated failure sending to {}", email.to));
    }
    if email.to.starts_with("slow") {
      tokio::time::sleep(TASK_EXECUTION_TIMEOUT + Duration::from_secs(15)).await;
    }

    tracing::info!(task_id = %record.id, to = %email.to, "sending email");
    let result = sonic_rs::to_string(&sonic_rs::json!({
      "delivered_to": email.to,
      "attempt": record.attempts,
    }))
    .map_err(|err| format!("encode task result: {err}"))?;
    Ok(Some(result))
  }
}

/// POST a JSON body to a URL; any non-2xx status is a failure (retry path).
/// Pointing `url` at a cluster node's own HTTP API chains tasks (e.g. a
/// webhook that enqueues an email task).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
  pub url: String,
  /// JSON body to send; defaults to `{"task_id": <id>}`.
  #[serde(default)]
  pub body: Option<sonic_rs::Value>,
}

pub struct WebhookHandler;

fn webhook_client() -> &'static reqwest::Client {
  static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
  CLIENT.get_or_init(|| {
    reqwest::Client::builder()
      .timeout(Duration::from_secs(10))
      .build()
      .expect("build webhook http client")
  })
}

#[async_trait::async_trait]
impl TypedTaskHandler for WebhookHandler {
  const KIND: &'static str = "webhook";
  type Payload = Webhook;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    webhook: Webhook,
  ) -> Result<Option<String>, String> {
    let body = webhook
      .body
      .unwrap_or_else(|| sonic_rs::json!({ "task_id": record.id }));
    tracing::info!(task_id = %record.id, url = %webhook.url, "delivering webhook");
    let response = webhook_client()
      .post(&webhook.url)
      .json(&body)
      .send()
      .await
      .map_err(|err| format!("webhook request to {} failed: {err}", webhook.url))?;
    let status = response.status();
    if !status.is_success() {
      return Err(format!(
        "webhook to {} returned status {status}",
        webhook.url
      ));
    }
    let result = sonic_rs::to_string(&sonic_rs::json!({
      "url": webhook.url,
      "status": status.as_u16(),
    }))
    .map_err(|err| format!("encode task result: {err}"))?;
    Ok(Some(result))
  }
}

/// CPU-bound checksum work (octopii-style durability verification): an
/// iterated SHA-256 chain over `data`, run on the blocking pool so it never
/// stalls the async executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestSpec {
  pub data: String,
  /// Number of chained hash rounds; capped at [`MAX_DIGEST_ITERATIONS`].
  #[serde(default = "default_digest_iterations")]
  pub iterations: u32,
}

fn default_digest_iterations() -> u32 {
  1
}

/// Keeps a runaway payload from monopolizing a blocking thread past the
/// execution timeout (~10M rounds is a few seconds).
pub const MAX_DIGEST_ITERATIONS: u32 = 10_000_000;

pub struct DigestHandler;

fn to_hex(bytes: &[u8]) -> String {
  bytes.iter().fold(
    String::with_capacity(bytes.len() * 2),
    |mut hex, byte| {
      let _ = write!(hex, "{byte:02x}");
      hex
    },
  )
}

#[async_trait::async_trait]
impl TypedTaskHandler for DigestHandler {
  const KIND: &'static str = "digest";
  type Payload = DigestSpec;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    spec: DigestSpec,
  ) -> Result<Option<String>, String> {
    if spec.iterations == 0 {
      return Err("digest iterations must be >= 1".to_string());
    }
    let iterations = spec.iterations.min(MAX_DIGEST_ITERATIONS);
    tracing::info!(task_id = %record.id, iterations, "computing digest");
    let digest = tokio::task::spawn_blocking(move || {
      let mut acc: Vec<u8> = spec.data.into_bytes();
      for _ in 0 .. iterations {
        acc = Sha256::digest(&acc).to_vec();
      }
      to_hex(&acc)
    })
    .await
    .map_err(|err| format!("digest worker panicked: {err}"))?;

    let result = sonic_rs::to_string(&sonic_rs::json!({
      "sha256": digest,
      "iterations": iterations,
    }))
    .map_err(|err| format!("encode task result: {err}"))?;
    Ok(Some(result))
  }
}

/// Write a key/value into a replicated KV group through raft — a task whose
/// side effect feeds back into the cluster (octopii's `propose` analogue).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvSet {
  pub key: String,
  pub value: String,
  /// Target raft group; defaults to the `users` KV group.
  #[serde(default = "default_kv_group")]
  pub group_id: String,
}

fn default_kv_group() -> String {
  groups::USERS.to_string()
}

pub struct KvSetHandler;

#[async_trait::async_trait]
impl TypedTaskHandler for KvSetHandler {
  const KIND: &'static str = "kv_set";
  type Payload = KvSet;

  async fn run(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    kv: KvSet,
  ) -> Result<Option<String>, String> {
    let cluster = ctx.cluster()?;
    tracing::info!(task_id = %record.id, group = %kv.group_id, key = %kv.key, "kv_set via raft");
    let reply = submit_reply(
      cluster.network,
      cluster.control_nodes,
      &kv.group_id,
      StateCommand::set(kv.key.clone(), kv.value),
    )
    .await
    .map_err(|err| format!("kv_set raft write failed: {err}"))?;

    let result = sonic_rs::to_string(&sonic_rs::json!({
      "group": kv.group_id,
      "key": kv.key,
      "stored": reply.value,
    }))
    .map_err(|err| format!("encode task result: {err}"))?;
    Ok(Some(result))
  }
}

/// Sleep for `secs`; drill knob for long-running work. Sleeping past the
/// worker execution timeout exercises the timeout/retry path on purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sleep {
  pub secs: u64,
}

pub struct SleepHandler;

#[async_trait::async_trait]
impl TypedTaskHandler for SleepHandler {
  const KIND: &'static str = "sleep";
  type Payload = Sleep;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    sleep: Sleep,
  ) -> Result<Option<String>, String> {
    tracing::info!(task_id = %record.id, secs = sleep.secs, "sleep task");
    tokio::time::sleep(Duration::from_secs(sleep.secs)).await;
    let result = sonic_rs::to_string(&sonic_rs::json!({ "slept_secs": sleep.secs }))
      .map_err(|err| format!("encode task result: {err}"))?;
    Ok(Some(result))
  }
}

#[cfg(test)]
mod tests {
  use std::time::Instant;

  use super::*;

  fn record(payload: &str) -> TaskRecord {
    TaskRecord {
      id: "task-test".to_string(),
      payload: payload.to_string(),
      status: crate::tasks::TaskStatus::Running,
      attempts: 1,
      run_at: 0,
      idem_key: None,
      assigned_node_id: None,
      lease_epoch: None,
      error: None,
      updated_at: 0,
      created_at: 0,
      completed_at: 0,
      result: None,
    }
  }

  #[test]
  fn payload_kind_tagged_legacy_and_invalid() {
    assert_eq!(
      payload_kind(r#"{"kind":"digest","data":"x"}"#).unwrap(),
      Some("digest".to_string())
    );
    assert_eq!(payload_kind(r#"{"to":"a@b"}"#).unwrap(), None);
    assert!(payload_kind("not json").is_err());
  }

  #[tokio::test]
  async fn legacy_untagged_payload_runs_email_handler() {
    let registry = TaskHandlerRegistry::builtin();
    let result = registry
      .execute(&TaskCtx::detached(), &record(r#"{"to":"legacy@example.com"}"#))
      .await
      .unwrap()
      .unwrap();
    assert!(result.contains("legacy@example.com"));
  }

  #[tokio::test]
  async fn tagged_email_payload_keeps_drill_semantics() {
    let registry = TaskHandlerRegistry::builtin();
    let err = registry
      .execute(
        &TaskCtx::detached(),
        &record(r#"{"kind":"email","to":"fail@example.com"}"#),
      )
      .await
      .unwrap_err();
    assert!(err.contains("simulated failure"));
  }

  #[tokio::test]
  async fn digest_matches_known_sha256() {
    let registry = TaskHandlerRegistry::builtin();
    let result = registry
      .execute(
        &TaskCtx::detached(),
        &record(r#"{"kind":"digest","data":"abc","iterations":1}"#),
      )
      .await
      .unwrap()
      .unwrap();
    // sha256("abc")
    assert!(result.contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
  }

  #[tokio::test]
  async fn unknown_kind_reports_registered_kinds() {
    let registry = TaskHandlerRegistry::builtin();
    let err = registry
      .execute(&TaskCtx::detached(), &record(r#"{"kind":"nope"}"#))
      .await
      .unwrap_err();
    assert!(err.contains("no handler registered"));
    assert!(err.contains("digest"));
  }

  #[tokio::test]
  async fn kv_set_without_cluster_access_fails_cleanly() {
    let registry = TaskHandlerRegistry::builtin();
    let err = registry
      .execute(
        &TaskCtx::detached(),
        &record(r#"{"kind":"kv_set","key":"k","value":"v"}"#),
      )
      .await
      .unwrap_err();
    assert!(err.contains("cluster access"));
  }

  /// A custom kind plugs in through the typed trait alone — the octopii-style
  /// extension point.
  #[tokio::test]
  async fn custom_handler_registers_and_dispatches() {
    #[derive(Deserialize)]
    struct Reverse {
      text: String,
    }
    struct ReverseHandler;

    #[async_trait::async_trait]
    impl TypedTaskHandler for ReverseHandler {
      const KIND: &'static str = "reverse";
      type Payload = Reverse;

      async fn run(
        &self,
        _ctx: &TaskCtx<'_>,
        _record: &TaskRecord,
        payload: Reverse,
      ) -> Result<Option<String>, String> {
        Ok(Some(payload.text.chars().rev().collect()))
      }
    }

    let mut registry = TaskHandlerRegistry::builtin();
    registry.register(Arc::new(ReverseHandler));
    let result = registry
      .execute(
        &TaskCtx::detached(),
        &record(r#"{"kind":"reverse","text":"abc"}"#),
      )
      .await
      .unwrap();
    assert_eq!(result.as_deref(), Some("cba"));
  }

  /// Kinds are not mutually exclusive: two different kinds run concurrently
  /// on one registry (well under the sum of their sequential durations).
  #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
  async fn different_kinds_run_concurrently() {
    let registry = Arc::new(TaskHandlerRegistry::builtin());
    let ctx = TaskCtx::detached();
    let rec_a = record(r#"{"kind":"sleep","secs":1}"#);
    let rec_b = record(r#"{"kind":"sleep","secs":1}"#);
    let rec_d = record(r#"{"kind":"digest","data":"overlap","iterations":1000}"#);
    let start = Instant::now();
    let sleep_a = registry.execute(&ctx, &rec_a);
    let sleep_b = registry.execute(&ctx, &rec_b);
    let digest = registry.execute(&ctx, &rec_d);
    let (a, b, d) = tokio::join!(sleep_a, sleep_b, digest);
    a.unwrap();
    b.unwrap();
    d.unwrap();
    // Sequentially this would take >2s; concurrently it settles in ~1s.
    assert!(start.elapsed() < Duration::from_millis(1900));
  }
}
