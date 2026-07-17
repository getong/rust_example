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
    scheduler::current_unix_secs,
    wasm_runtime,
    worker::{TASK_EXECUTION_TIMEOUT, submit_command, submit_reply},
  },
  types_kv::TaskRequest as StateCommand,
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
  /// The handler itself stored as data: a WebAssembly module carried in the
  /// payload (and thus in the raft log), executed under wasmtime + WASI.
  Wasm(WasmExec),
}

/// ONE table binding each kind tag to its variant and payload type.
/// `kind()`, `decode()` and the known-kinds list are all generated from it,
/// so the tag ↔ variant mapping cannot drift between them (the failure mode
/// of maintaining three hand-written `match`es). The serde tag on the enum
/// is pinned to this table by the `all_variants_roundtrip_with_kind_tag`
/// test. Adding a task type = one new table row (+ the `execute` arm, which
/// the compiler's exhaustiveness check enforces).
macro_rules! task_payload_kinds {
  ($(($kind:literal, $variant:ident, $ty:ty)),+ $(,)?) => {
    impl TaskPayload {
      /// Kind tags understood by [`TaskPayload::decode`].
      pub const KNOWN_KINDS: &'static [&'static str] = &[$($kind),+];

      /// The `kind` tag of this payload (matches the serde tag).
      pub fn kind(&self) -> &'static str {
        match self {
          $(TaskPayload::$variant(_) => $kind,)+
        }
      }

      /// Decode a stored payload: read the `kind` tag, then decode the
      /// matching variant struct (the extra `kind` field is ignored by
      /// serde). A JSON object without a tag is the legacy email shape; an
      /// unknown kind fails here listing the known ones.
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
          // Legacy untagged payloads predate the kind tag: email shape.
          None => variant::<Email>("email", payload, TaskPayload::Email),
          $(Some($kind) => variant::<$ty>($kind, payload, TaskPayload::$variant),)+
          Some(other) => Err(format!(
            "unknown task kind {other:?} (known: {})",
            Self::KNOWN_KINDS.join(", ")
          )),
        }
      }
    }
  };
}

task_payload_kinds! {
  ("email", Email, Email),
  ("webhook", Webhook, Webhook),
  ("digest", Digest, DigestSpec),
  ("kv_set", KvSet, KvSet),
  ("sleep", Sleep, Sleep),
  ("wasm", Wasm, WasmExec),
}

impl TaskPayload {
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
      TaskPayload::Wasm(wasm) => format!("wasm:{}", wasm.display_name()),
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
      TaskPayload::Wasm(wasm) => WasmExecHandler.run(ctx, record, wasm).await,
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

  /// Declare `record` past its point of no return via a replicated
  /// `TaskMarkCommitted` write (renegade's `Running.committed` pattern).
  ///
  /// MUST be called — and must return Ok — immediately BEFORE an
  /// irreversible side effect. An Err means the write was rejected: the task
  /// was requeued or reassigned in the meantime, and the handler must abort
  /// (return the Err) instead of executing the side effect — winning this
  /// raft write is what resolves the race. After a successful mark, the
  /// state machine will never re-queue the task (a requeue proposal fails it
  /// terminally for reconciliation) and a generic failure/timeout suppresses
  /// the retry, so a duplicate side effect cannot be scheduled.
  ///
  /// Detached contexts (unit tests, standalone tools) skip the mark.
  pub async fn mark_committed(&self, record: &TaskRecord) -> Result<(), String> {
    let Some(cluster) = self.cluster.as_ref() else {
      return Ok(());
    };
    let node_id = record
      .assigned_node_id
      .clone()
      .ok_or_else(|| "commit-mark: record has no assigned node".to_string())?;
    let lease_epoch = record
      .lease_epoch
      .ok_or_else(|| "commit-mark: record has no lease epoch".to_string())?;

    let result = submit_command(
      cluster.network,
      cluster.control_nodes,
      groups::TASKS,
      StateCommand::TaskMarkCommitted {
        id: record.id.clone(),
        node_id,
        lease_epoch,
        now: current_unix_secs(),
      },
    )
    .await
    .map_err(|err| format!("commit-mark raft write failed: {err}"))?;

    if !result.ok {
      return Err(format!(
        "commit-mark rejected (task requeued/reassigned?), aborting side effect: {:?}",
        result.reason
      ));
    }
    Ok(())
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
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    email: &Email,
  ) -> Result<Option<String>, String> {
    // Testable failure semantics for drills: recipients starting with "fail"
    // simulate a handler error (exercises retry/backoff and permanent
    // failure), "slow" simulates a hang (exercises the execution timeout).
    // Both fire BEFORE the commit mark so the drills keep exercising the
    // plain retry path.
    if email.to.starts_with("fail") {
      return Err(format!("simulated failure sending to {}", email.to));
    }
    if email.to.starts_with("slow") {
      tokio::time::sleep(TASK_EXECUTION_TIMEOUT + Duration::from_secs(15)).await;
    }

    // Sending is irreversible: declare the commit point first, abort on loss.
    ctx.mark_committed(record).await?;

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
    ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    webhook: &Webhook,
  ) -> Result<Option<String>, String> {
    let body = webhook
      .body
      .clone()
      .unwrap_or_else(|| sonic_rs::json!({ "task_id": record.id }));

    // The POST is an irreversible external side effect: declare the commit
    // point first, abort on loss.
    ctx.mark_committed(record).await?;

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
      crate::types_kv::Request::set(kv.key.clone(), kv.value.clone()),
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

/// The handler function stored as DATA or as a FILE:
///
/// - Code-as-data: the task carries the WebAssembly module itself — WAT text (`module_wat`) or a
///   base64 wasm binary (`module_b64`) — so the processing logic is replicated through the raft log
///   alongside the arguments.
/// - Code-as-file (docker-like): the task carries only a `module_file` reference into the worker's
///   [`wasm_runtime::WasmModuleStore`] directory — the module is a `.wasm`/`.wat` file deployed
///   like a docker image, and the raft log stays small. An optional `module_sha256` pins the file's
///   content digest (docker digest-pinning) so a swapped file is refused instead of silently
///   executed.
///
/// Execution goes through the engine-agnostic
/// [`wasm_runtime::WasmRuntime`] trait (selected via `WASM_RUNTIME`, today
/// wasmtime's WASI p2 component runtime): the guest gets argv/env, its
/// stdout is captured (bounded) as the task result, and a fuel budget
/// terminates runaway modules deterministically — execution stays on the
/// worker, never inside `apply()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExec {
  /// Human-readable WAT text (core module or component syntax).
  /// Exactly one of `module_wat` / `module_b64` / `module_file`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub module_wat: Option<String>,
  /// Base64-encoded wasm binary: a p2 component (wasm32-wasip2 output) or
  /// a p1 core module (wasm32-wasip1) — both are accepted.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub module_b64: Option<String>,
  /// Bare file name of a module in the worker's module store directory
  /// (`WASM_MODULES_DIR`, default `wasm_modules/`); `<name>`, `<name>.wasm`
  /// and `<name>.wat` are tried in that order. Path separators and `..`
  /// are rejected.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub module_file: Option<String>,
  /// Optional content-digest pin (hex sha256 of the module bytes, with or
  /// without a `sha256:` prefix). Execution is refused on mismatch.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub module_sha256: Option<String>,
  /// argv[1..] for the guest (argv[0] is always "task.wasm").
  #[serde(default)]
  pub args: Vec<String>,
  /// Environment variables for the guest.
  #[serde(default)]
  pub env: std::collections::BTreeMap<String, String>,
  /// Display name for lists/logs (defaults to the file reference or the
  /// module's content hash).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

impl WasmExec {
  fn display_name(&self) -> String {
    if let Some(name) = &self.name {
      return name.clone();
    }
    if let Some(file) = &self.module_file {
      return format!("file:{file}");
    }
    match self.module_source() {
      Ok(bytes) => format!("sha256:{}", &wasm_runtime::module_hash(&bytes)[.. 12]),
      Err(_) => "<invalid module>".to_string(),
    }
  }

  /// Raw module bytes from exactly one source: WAT text as-is, the decoded
  /// base64 binary, or the referenced file in the module store. Verifies
  /// the `module_sha256` digest pin when present.
  fn module_source(&self) -> Result<Vec<u8>, String> {
    let bytes = match (&self.module_wat, &self.module_b64, &self.module_file) {
      (Some(wat), None, None) => wat.clone().into_bytes(),
      (None, Some(b64), None) => {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
          .decode(b64)
          .map_err(|err| format!("decode module_b64: {err}"))?
      }
      (None, None, Some(file)) => wasm_runtime::WasmModuleStore::from_env().load(file)?,
      (None, None, None) => {
        return Err("wasm task needs module_wat, module_b64 or module_file".to_string());
      }
      _ => {
        return Err(
          "set exactly one of module_wat / module_b64 / module_file, not several".to_string(),
        );
      }
    };

    if let Some(pin) = &self.module_sha256 {
      let expected = pin.strip_prefix("sha256:").unwrap_or(pin).to_lowercase();
      let actual = wasm_runtime::module_hash(&bytes);
      if actual != expected {
        return Err(format!(
          "module digest mismatch: payload pins sha256:{expected}, module is sha256:{actual}"
        ));
      }
    }
    Ok(bytes)
  }
}

/// Executes [`WasmExec`] payloads on the engine selected by `WASM_RUNTIME`
/// (see [`wasm_runtime::selected_runtime`]). Holding the engine as a trait
/// object keeps this handler engine-agnostic: a future wasmer backend plugs
/// in through the registry without touching this code.
pub struct WasmExecHandler;

#[async_trait::async_trait]
impl TaskHandler for WasmExecHandler {
  type Payload = WasmExec;

  async fn run(
    &self,
    _ctx: &TaskCtx<'_>,
    record: &TaskRecord,
    wasm: &WasmExec,
  ) -> Result<Option<String>, String> {
    let runtime = wasm_runtime::selected_runtime()?;
    let source = wasm.module_source()?;
    let module_hash = wasm_runtime::module_hash(&source);
    let invocation = wasm_runtime::WasmInvocation {
      args: wasm.args.clone(),
      env: wasm.env.clone().into_iter().collect(),
      fuel_limit: Some(wasm_runtime::WASM_FUEL_LIMIT),
      // Epoch backstop just past the task timeout: even if this future is
      // dropped on timeout, the guest is interrupted and the executor
      // thread is released instead of spinning to fuel exhaustion.
      wall_clock_limit: Some(TASK_EXECUTION_TIMEOUT + Duration::from_secs(5)),
    };
    tracing::info!(
      task_id = %record.id,
      runtime = runtime.name(),
      module_hash = %module_hash[.. 12],
      module_bytes = source.len(),
      "executing wasm task"
    );

    // Wasm execution is CPU-bound and the guest host calls are sync: run
    // on the dedicated wasm executor pool (isolated from tokio's blocking
    // pool); fuel + the epoch deadline bound the runtime.
    let outcome = wasm_runtime::execute_pooled(runtime, source, invocation).await?;

    let result = sonic_rs::to_string(&sonic_rs::json!({
      "stdout": outcome.stdout,
      "fuel_used": outcome.fuel_used,
      "structured": outcome.structured,
      "module": wasm.display_name(),
      "runtime": runtime.name(),
    }))
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
      committed: false,
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
      TaskPayload::Wasm(WasmExec {
        module_wat: Some("(module)".to_string()),
        module_b64: None,
        module_file: None,
        module_sha256: None,
        args: vec!["x".to_string()],
        env: Default::default(),
        name: Some("noop".to_string()),
      }),
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

  /// A hand-written WASI module: the handler function stored as data.
  /// Writes "hello from wasm task\n" (21 bytes) to fd 1 via fd_write.
  #[cfg(feature = "p1-compat")]
  const HELLO_WAT: &str = r#"
    (module
      (import "wasi_snapshot_preview1" "fd_write"
        (func $fd_write (param i32 i32 i32 i32) (result i32)))
      (memory 1)
      (export "memory" (memory 0))
      (data (i32.const 8) "hello from wasm task\n")
      (func (export "_start")
        (i32.store (i32.const 0) (i32.const 8))   ;; iov.base
        (i32.store (i32.const 4) (i32.const 21))  ;; iov.len
        (drop (call $fd_write
          (i32.const 1)   ;; fd = stdout
          (i32.const 0)   ;; iovs ptr
          (i32.const 1)   ;; iovs len
          (i32.const 32)  ;; nwritten out-ptr
        ))))
  "#;

  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_module_runs_and_captures_stdout() {
    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: Some(HELLO_WAT.to_string()),
      module_b64: None,
      module_file: None,
      module_sha256: None,
      args: Vec::new(),
      env: Default::default(),
      name: Some("hello".to_string()),
    })
    .encode()
    .unwrap();
    let result = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap()
      .unwrap();
    assert!(
      result.contains("hello from wasm task"),
      "stdout missing from result: {result}"
    );
    assert!(result.contains("fuel_used"), "no fuel report: {result}");
  }

  /// The direct p2 path: a real component binary (here produced by the
  /// same adapter used at load time) submitted via `module_b64` runs
  /// without re-adaptation.
  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_component_binary_runs_directly() {
    use base64::Engine as _;
    let component = wasm_runtime::componentize(HELLO_WAT.as_bytes()).expect("componentize");
    assert!(
      wasm_runtime::is_component_binary(&component),
      "componentize must produce a component binary"
    );
    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: None,
      module_b64: Some(base64::engine::general_purpose::STANDARD.encode(component)),
      module_file: None,
      module_sha256: None,
      args: Vec::new(),
      env: Default::default(),
      name: Some("hello-component".to_string()),
    })
    .encode()
    .unwrap();
    let result = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap()
      .unwrap();
    assert!(
      result.contains("hello from wasm task"),
      "stdout missing from result: {result}"
    );
  }

  /// The richer end-to-end example shared with the test script: a stored
  /// handler that PARSES argv, COMPUTES (trial-division prime counting)
  /// and FORMATS its own output. π(1000) = 168.
  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_prime_count_computes_from_argv() {
    let wat = std::fs::read_to_string(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/script/wat/prime_count.wat"
    ))
    .expect("read prime_count.wat");
    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: Some(wat),
      module_b64: None,
      module_file: None,
      module_sha256: None,
      args: vec!["1000".to_string()],
      env: Default::default(),
      name: Some("prime-count".to_string()),
    })
    .encode()
    .unwrap();
    let result = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap()
      .unwrap();
    let parsed: sonic_rs::Value = sonic_rs::from_str(&result).unwrap();
    use sonic_rs::JsonValueTrait as _;
    assert_eq!(
      parsed.get("stdout").as_str(),
      Some("168\n"),
      "unexpected result: {result}"
    );
  }

  /// The richest shared example: stats.wat folds argv numbers into
  /// count/sum/min/max/avg, reads a LABEL env var, and assembles a JSON
  /// report INSIDE the guest — the task result's stdout is itself JSON.
  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_stats_builds_json_report_from_argv_and_env() {
    let wat = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/script/wat/stats.wat"))
      .expect("read stats.wat");
    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: Some(wat),
      module_b64: None,
      module_file: None,
      module_sha256: None,
      args: vec!["12", "7", "42", "19"]
        .into_iter()
        .map(String::from)
        .collect(),
      env: [("LABEL".to_string(), "unit-test".to_string())].into(),
      name: Some("stats".to_string()),
    })
    .encode()
    .unwrap();
    let result = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap()
      .unwrap();

    use sonic_rs::{JsonValueTrait as _, Value};
    let outer: Value = sonic_rs::from_str(&result).unwrap();
    let stdout_value = outer.get("stdout");
    let stdout = stdout_value.as_str().expect("stdout in result");
    let report: Value = sonic_rs::from_str(stdout.trim()).expect("guest stdout is JSON");
    assert_eq!(report.get("label").as_str(), Some("unit-test"));
    assert_eq!(report.get("count").as_i64(), Some(4));
    assert_eq!(report.get("sum").as_i64(), Some(80));
    assert_eq!(report.get("min").as_i64(), Some(7));
    assert_eq!(report.get("max").as_i64(), Some(42));
    assert_eq!(report.get("avg").as_i64(), Some(20));
  }

  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_infinite_loop_is_stopped_by_fuel() {
    // The preview1 command adapter requires the core module to export its
    // memory, even for a pure spin loop.
    let spin_wat = r#"
      (module
        (memory 1)
        (export "memory" (memory 0))
        (func (export "_start") (loop br 0)))
    "#;
    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: Some(spin_wat.to_string()),
      module_b64: None,
      module_file: None,
      module_sha256: None,
      args: Vec::new(),
      env: Default::default(),
      name: Some("spin".to_string()),
    })
    .encode()
    .unwrap();
    let err = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap_err();
    assert!(
      err.contains("fuel") || err.contains("trap"),
      "expected fuel trap, got: {err}"
    );
  }

  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_nonzero_exit_is_a_failure() {
    // WASI p1 host functions require the guest to export a `memory`, even
    // for proc_exit, so declare one.
    let wat = r#"
      (module
        (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
        (memory 1)
        (export "memory" (memory 0))
        (func (export "_start") (call $exit (i32.const 7))))
    "#;
    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: Some(wat.to_string()),
      module_b64: None,
      module_file: None,
      module_sha256: None,
      args: Vec::new(),
      env: Default::default(),
      name: None,
    })
    .encode()
    .unwrap();
    let err = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap_err();
    // WASI 0.2's `exit` only carries success/failure, so the preview1
    // adapter collapses proc_exit(7) to a generic non-zero exit — what
    // matters is that it fails and reports an exit, not the exact code.
    assert!(err.contains("exited with code"), "unexpected error: {err}");
  }

  #[tokio::test]
  async fn wasm_without_module_fails_at_decode_time() {
    let err = execute_payload(&TaskCtx::detached(), &record(r#"{"kind":"wasm"}"#))
      .await
      .unwrap_err();
    assert!(
      err.contains("module_wat, module_b64 or module_file"),
      "unexpected error: {err}"
    );
  }

  /// Docker-like code-as-file: the module lives as a file in the store
  /// directory; the payload carries only its name (plus an optional digest
  /// pin). Store dir, execution, and a swapped-file digest mismatch are all
  /// exercised in ONE test so the env var is touched from a single place.
  #[cfg(feature = "p1-compat")]
  #[tokio::test]
  async fn wasm_module_file_runs_from_store_and_digest_pin_protects() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.wat"), HELLO_WAT).unwrap();
    // SAFETY: single-threaded moment for this test; no other test reads
    // WASM_MODULES_DIR (module_file is exercised only here).
    unsafe { std::env::set_var(wasm_runtime::WASM_MODULES_DIR_ENV, dir.path()) };

    let payload = TaskPayload::Wasm(WasmExec {
      module_wat: None,
      module_b64: None,
      module_file: Some("hello".to_string()),
      module_sha256: Some(wasm_runtime::module_hash(HELLO_WAT.as_bytes())),
      args: Vec::new(),
      env: Default::default(),
      name: None,
    })
    .encode()
    .unwrap();
    // The payload is tiny — the raft log carries the reference, not the code.
    assert!(payload.len() < 256, "reference payload should stay small");
    let result = execute_payload(&TaskCtx::detached(), &record(&payload))
      .await
      .unwrap()
      .unwrap();
    assert!(
      result.contains("hello from wasm task"),
      "stdout missing from result: {result}"
    );
    assert!(
      result.contains("file:hello"),
      "file label missing: {result}"
    );

    // Same reference with a stale digest pin: the swapped module is refused.
    let pinned = TaskPayload::Wasm(WasmExec {
      module_wat: None,
      module_b64: None,
      module_file: Some("hello".to_string()),
      module_sha256: Some(format!("sha256:{}", wasm_runtime::module_hash(b"other"))),
      args: Vec::new(),
      env: Default::default(),
      name: None,
    })
    .encode()
    .unwrap();
    let err = execute_payload(&TaskCtx::detached(), &record(&pinned))
      .await
      .unwrap_err();
    assert!(err.contains("digest mismatch"), "unexpected error: {err}");
  }

  #[tokio::test]
  async fn wasm_with_two_module_sources_is_rejected() {
    let err = execute_payload(
      &TaskCtx::detached(),
      &record(r#"{"kind":"wasm","module_wat":"(module)","module_file":"hello"}"#),
    )
    .await
    .unwrap_err();
    assert!(err.contains("exactly one of"), "unexpected error: {err}");
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
