//! Multi-kind task execution unified around one enum, octopii-style: like
//! octopii's `StateMachineTrait`, the worker pipeline knows nothing about
//! concrete tasks — it decodes a [`TaskPayload`] and calls
//! [`TaskPayload::execute`].
//!
//! A payload is self-describing JSON tagged with `kind`
//! (`{"kind":"email","to":"a@b"}`); payloads written before the tag existed
//! (bare `{"to":..}`) still decode as [`TaskPayload::Email`].
//!
//! Adding a task type = append a variant to [`TaskPayload`], implement a
//! [`TaskHandler`] for its payload struct, and add the one `match` arm in
//! [`TaskPayload::execute`] — the compiler's exhaustiveness check makes a
//! forgotten arm a build error. Claim/execute/ack, retry/backoff, timeouts
//! and concurrency are shared. Kinds are NOT mutually exclusive: dispatch is
//! per task, so one worker interleaves any mix of kinds up to its
//! execution-permit cap.

use std::{fmt::Write as _, time::Duration};

use serde::{Deserialize, Serialize};
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

/// Every task kind understood by workers, unified in one tagged enum. The
/// serde tag is the payload's `kind` field, so the JSON wire format is
/// `{"kind":"digest","data":"..."}`. Decoding is hand-rolled in
/// [`TaskPayload::decode`] (kind tag first, then the variant struct) because
/// sonic_rs cannot drive serde's internally-tagged-enum buffering through a
/// nested [`sonic_rs::Value`].
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskPayload {
  /// The original demo task: pretend-send an email.
  Email(Email),
  /// POST a JSON body to a URL (task chaining when pointed at the cluster).
  Webhook(Webhook),
  /// CPU-bound iterated SHA-256 (octopii-style durability verification).
  Digest(DigestSpec),
  /// Raft KV write-back into a replicated group (octopii's `propose`).
  KvSet(KvSet),
  /// Timed no-op; drill knob for long-running work.
  Sleep(Sleep),
}

impl TaskPayload {
  /// The `kind` tag of this payload (matches the serde tag).
  pub fn kind(&self) -> &'static str {
    match self {
      TaskPayload::Email(_) => "email",
      TaskPayload::Webhook(_) => "webhook",
      TaskPayload::Digest(_) => "digest",
      TaskPayload::KvSet(_) => "kv_set",
      TaskPayload::Sleep(_) => "sleep",
    }
  }

  /// Decode a stored payload: read the `kind` tag, then decode the matching
  /// variant struct (the extra `kind` field is ignored by serde). A JSON
  /// object without a tag is the legacy email shape; an unknown kind fails
  /// here listing the known ones. New variants MUST add their arm — like
  /// `execute`, the match is exhaustive over the kinds.
  pub fn decode(payload: &str) -> Result<Self, String> {
    fn variant<T: serde::de::DeserializeOwned>(
      kind: &str,
      payload: &str,
      wrap: fn(T) -> TaskPayload,
    ) -> Result<TaskPayload, String> {
      sonic_rs::from_str::<T>(payload)
        .map(wrap)
        .map_err(|err| format!("decode {kind} payload: {err}"))
    }

    match payload_kind(payload)?.as_deref() {
      None | Some("email") => variant("email", payload, TaskPayload::Email),
      Some("webhook") => variant("webhook", payload, TaskPayload::Webhook),
      Some("digest") => variant("digest", payload, TaskPayload::Digest),
      Some("kv_set") => variant("kv_set", payload, TaskPayload::KvSet),
      Some("sleep") => variant("sleep", payload, TaskPayload::Sleep),
      Some(other) => Err(format!(
        "unknown task kind {other:?} (known: email, webhook, digest, kv_set, sleep)"
      )),
    }
  }

  /// Encode as kind-tagged JSON (the storage/wire format).
  pub fn encode(&self) -> Result<String, String> {
    sonic_rs::to_string(self).map_err(|err| format!("encode task payload: {err}"))
  }

  /// Compact `kind:detail` label for lists and logs.
  pub fn label(&self) -> String {
    match self {
      TaskPayload::Email(email) => format!("email:{}", email.to),
      TaskPayload::Webhook(webhook) => format!("webhook:{}", webhook.url),
      TaskPayload::Digest(spec) => format!("digest:{}", spec.data),
      TaskPayload::KvSet(kv) => format!("kv_set:{}", kv.key),
      TaskPayload::Sleep(sleep) => format!("sleep:{}s", sleep.secs),
    }
  }

  /// Dispatch to the kind's handler. `Ok(result)` is stored on the record
  /// as opaque JSON via `TaskDone`; `Err` goes through the shared
  /// retry/backoff path. New variants MUST add their arm here — the match
  /// is deliberately exhaustive (no `_`).
  pub async fn execute(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
  ) -> Result<Option<String>, String> {
    match self {
      TaskPayload::Email(email) => EmailHandler.run(ctx, record, email).await,
      TaskPayload::Webhook(webhook) => WebhookHandler.run(ctx, record, webhook).await,
      TaskPayload::Digest(spec) => DigestHandler.run(ctx, record, spec).await,
      TaskPayload::KvSet(kv) => KvSetHandler.run(ctx, record, kv).await,
      TaskPayload::Sleep(sleep) => SleepHandler.run(ctx, record, sleep).await,
    }
  }
}

/// Decode and execute a raw stored payload in one step (the worker's entry
/// point).
pub async fn execute_payload(
  ctx: &TaskCtx<'_>,
  record: &TaskRecord,
) -> Result<Option<String>, String> {
  TaskPayload::decode(&record.payload)?
    .execute(ctx, record)
    .await
}

/// Extract the `kind` tag from a payload. `Ok(None)` means the payload is a
/// JSON object without a tag (legacy email).
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

/// Uniform signature for one kind's execution logic. Purely an organization
/// aid: dispatch is the exhaustive `match` in [`TaskPayload::execute`], so
/// implementing this trait alone does nothing until the variant + arm exist.
#[async_trait::async_trait]
pub trait TaskHandler: Send + Sync {
  type Payload: Send + Sync;

  async fn run(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    payload: &Self::Payload,
  ) -> Result<Option<String>, String>;
}

// ---------------------------------------------------------------------------
// Per-kind payload structs + handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
  pub to: String,
}

pub struct EmailHandler;

#[async_trait::async_trait]
impl TaskHandler for EmailHandler {
  type Payload = Email;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    email: &Email,
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
impl TaskHandler for WebhookHandler {
  type Payload = Webhook;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    webhook: &Webhook,
  ) -> Result<Option<String>, String> {
    let body = webhook
      .body
      .clone()
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
  bytes
    .iter()
    .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
      let _ = write!(hex, "{byte:02x}");
      hex
    })
}

#[async_trait::async_trait]
impl TaskHandler for DigestHandler {
  type Payload = DigestSpec;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    spec: &DigestSpec,
  ) -> Result<Option<String>, String> {
    if spec.iterations == 0 {
      return Err("digest iterations must be >= 1".to_string());
    }
    let iterations = spec.iterations.min(MAX_DIGEST_ITERATIONS);
    let data = spec.data.clone();
    tracing::info!(task_id = %record.id, iterations, "computing digest");
    let digest = tokio::task::spawn_blocking(move || {
      let mut acc: Vec<u8> = data.into_bytes();
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
impl TaskHandler for KvSetHandler {
  type Payload = KvSet;

  async fn run(
    &self,
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    kv: &KvSet,
  ) -> Result<Option<String>, String> {
    let cluster = ctx.cluster()?;
    tracing::info!(task_id = %record.id, group = %kv.group_id, key = %kv.key, "kv_set via raft");
    let reply = submit_reply(
      cluster.network,
      cluster.control_nodes,
      &kv.group_id,
      StateCommand::set(kv.key.clone(), kv.value.clone()),
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
impl TaskHandler for SleepHandler {
  type Payload = Sleep;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    sleep: &Sleep,
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
  use std::time::{Duration, Instant};

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

  /// Every variant encodes to kind-tagged JSON and decodes back — the
  /// checklist to extend when appending a new kind to the enum.
  #[test]
  fn all_variants_roundtrip_with_kind_tag() {
    let variants = vec![
      TaskPayload::Email(Email {
        to: "a@b".to_string(),
      }),
      TaskPayload::Webhook(Webhook {
        url: "http://127.0.0.1:1/x".to_string(),
        body: Some(sonic_rs::json!({"k":"v"})),
      }),
      TaskPayload::Digest(DigestSpec {
        data: "abc".to_string(),
        iterations: 3,
      }),
      TaskPayload::KvSet(KvSet {
        key: "k".to_string(),
        value: "v".to_string(),
        group_id: "users".to_string(),
      }),
      TaskPayload::Sleep(Sleep { secs: 1 }),
    ];
    for payload in variants {
      let encoded = payload.encode().unwrap();
      assert!(
        encoded.contains(&format!(r#""kind":"{}""#, payload.kind())),
        "tag missing in {encoded}"
      );
      let decoded = TaskPayload::decode(&encoded).unwrap();
      assert_eq!(decoded.kind(), payload.kind());
      assert_eq!(decoded.label(), payload.label());
    }
  }

  #[test]
  fn unknown_kind_fails_at_decode_listing_variants() {
    let err = TaskPayload::decode(r#"{"kind":"nope"}"#).unwrap_err();
    assert!(err.contains("nope"), "unexpected error: {err}");
    assert!(err.contains("email"), "should list known kinds: {err}");
  }

  #[tokio::test]
  async fn legacy_untagged_payload_runs_email_handler() {
    let result = execute_payload(
      &TaskCtx::detached(),
      &record(r#"{"to":"legacy@example.com"}"#),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.contains("legacy@example.com"));
  }

  #[tokio::test]
  async fn tagged_email_payload_keeps_drill_semantics() {
    let err = execute_payload(
      &TaskCtx::detached(),
      &record(r#"{"kind":"email","to":"fail@example.com"}"#),
    )
    .await
    .unwrap_err();
    assert!(err.contains("simulated failure"));
  }

  #[tokio::test]
  async fn digest_matches_known_sha256() {
    let result = execute_payload(
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
  async fn kv_set_without_cluster_access_fails_cleanly() {
    let err = execute_payload(
      &TaskCtx::detached(),
      &record(r#"{"kind":"kv_set","key":"k","value":"v"}"#),
    )
    .await
    .unwrap_err();
    assert!(err.contains("cluster access"));
  }

  /// Kinds are not mutually exclusive: different kinds run concurrently
  /// (well under the sum of their sequential durations).
  #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
  async fn different_kinds_run_concurrently() {
    let ctx = TaskCtx::detached();
    let rec_a = record(r#"{"kind":"sleep","secs":1}"#);
    let rec_b = record(r#"{"kind":"sleep","secs":1}"#);
    let rec_d = record(r#"{"kind":"digest","data":"overlap","iterations":1000}"#);
    let start = Instant::now();
    let (a, b, d) = tokio::join!(
      execute_payload(&ctx, &rec_a),
      execute_payload(&ctx, &rec_b),
      execute_payload(&ctx, &rec_d),
    );
    a.unwrap();
    b.unwrap();
    d.unwrap();
    // Sequentially this would take >2s; concurrently it settles in ~1s.
    assert!(start.elapsed() < Duration::from_millis(1900));
  }
}
