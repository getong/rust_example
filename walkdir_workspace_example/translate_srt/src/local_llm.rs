use std::{collections::VecDeque, num::NonZeroU32, path::PathBuf, thread};

use anyhow::{Context, Result, anyhow};
use encoding_rs::{Decoder, UTF_8};
use llama_cpp_2::{
  LogOptions,
  context::{LlamaContext, params::LlamaContextParams},
  llama_backend::LlamaBackend,
  llama_batch::LlamaBatch,
  model::{AddBos, LlamaChatMessage, LlamaModel, params::LlamaModelParams},
  send_logs_to_tracing,
  token::LlamaToken,
};
use tokio::sync::{mpsc, oneshot};

use crate::TranslationFormat;

/// Largest prompt-plus-reply a single request may occupy. Requests above this are rejected so the
/// caller can split them.
const SLOT_CONTEXT_TOKENS: u32 = 3_072;
/// KV cells budgeted per slot.
///
/// The cache is one shared pool, and slots are admitted against it rather than each reserving the
/// worst case, so this only has to cover a typical request. A slot holding an unusually long
/// request simply borrows the headroom other slots are not using.
const POOL_TOKENS_PER_SLOT: u32 = 512;
/// Generated tokens allowed per request, as a multiple of the prompt length.
const MAX_NEW_TOKENS_PER_PROMPT_TOKEN: usize = 2;
/// Formats whose shared prompt prefix is kept resident in the KV cache.
const PREFIX_FORMATS: [TranslationFormat; 2] = [TranslationFormat::Text, TranslationFormat::Html];
/// Extra KV cells reserved for those resident prefixes.
const PREFIX_RESERVE_TOKENS: u32 = 1_024;
/// Shortest shared prefix worth reusing instead of prefilling from scratch.
const MIN_SHARED_PREFIX_TOKENS: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum LocalLlmError {
  #[error("本地 GGUF 推理引擎已停止")]
  Shutdown,
}

/// Handle to the GGUF inference engine.
///
/// The engine owns the model and a single llama.cpp context on a dedicated thread. Requests are
/// queued and decoded in parallel across `slots` independent sequences, so one forward pass
/// produces one token for every in-flight request instead of just one. Single-stream decoding of a
/// 4B model is memory-bandwidth bound, so batching sequences is close to free throughput.
#[derive(Debug)]
pub struct LocalTranslator {
  /// Dropped on shutdown to tell the engine thread to finish and release the llama.cpp context.
  jobs: Option<mpsc::UnboundedSender<Job>>,
  engine: Option<thread::JoinHandle<()>>,
  slots: usize,
}

#[derive(Debug)]
struct Job {
  system: String,
  user: String,
  format: TranslationFormat,
  reply: oneshot::Sender<Result<String>>,
}

impl LocalTranslator {
  pub fn new(model_path: PathBuf, cpu: bool, verbose: bool, slots: Option<usize>) -> Result<Self> {
    let (jobs_sender, jobs_receiver) = mpsc::unbounded_channel();
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();

    let engine = thread::Builder::new()
      .name("gguf-engine".to_owned())
      .spawn(move || {
        engine_main(
          model_path,
          cpu,
          verbose,
          slots,
          &ready_sender,
          jobs_receiver,
        );
      })
      .context("无法启动本地 GGUF 推理线程")?;

    match ready_receiver.recv() {
      Ok(Ok(slots)) => Ok(Self {
        jobs: Some(jobs_sender),
        engine: Some(engine),
        slots,
      }),
      Ok(Err(error)) => Err(error),
      Err(_) => Err(anyhow!("本地 GGUF 推理线程在初始化时意外退出")),
    }
  }

  /// Number of requests the engine decodes concurrently.
  pub fn slots(&self) -> usize {
    self.slots
  }

  pub async fn translate(&self, input: &str, format: TranslationFormat) -> Result<String> {
    let prompt = TranslationPrompt::new(format).build(input);
    let (reply_sender, reply_receiver) = oneshot::channel();

    self
      .jobs
      .as_ref()
      .ok_or(LocalLlmError::Shutdown)?
      .send(Job {
        system: prompt.system,
        user: prompt.user,
        format,
        reply: reply_sender,
      })
      .map_err(|_| LocalLlmError::Shutdown)?;

    reply_receiver.await.map_err(|_| LocalLlmError::Shutdown)?
  }
}

impl Drop for LocalTranslator {
  /// Lets the engine thread drop the llama.cpp context before the process exits.
  ///
  /// Metal asserts during its atexit teardown if a context is still holding residency sets, so the
  /// context has to be released while the runtime is still alive.
  fn drop(&mut self) {
    self.jobs = None;
    if let Some(engine) = self.engine.take() {
      let _ = engine.join();
    }
  }
}

#[allow(unused_mut)]
fn vram_mib() -> Option<(u64, u64)> {
  let mut free = 0usize;
  let mut total = 0usize;

  #[cfg(feature = "cuda")]
  {
    unsafe extern "C" {
      fn ggml_backend_cuda_get_device_memory(device: i32, free: *mut usize, total: *mut usize);
    }
    unsafe { ggml_backend_cuda_get_device_memory(0, &mut free, &mut total) };
    if total > 0 {
      return Some((free as u64 / (1024 * 1024), total as u64 / (1024 * 1024)));
    }
  }

  #[cfg(feature = "vulkan")]
  {
    unsafe extern "C" {
      fn ggml_backend_vk_get_device_memory(device: i32, free: *mut usize, total: *mut usize);
    }
    unsafe { ggml_backend_vk_get_device_memory(0, &mut free, &mut total) };
    if total > 0 {
      return Some((free as u64 / (1024 * 1024), total as u64 / (1024 * 1024)));
    }
  }

  let _ = (free, total);
  None
}

fn pick_n_ubatch(use_gpu: bool) -> u32 {
  let default = LlamaContextParams::default().n_ubatch();
  if use_gpu && let Some((_, total_mib)) = vram_mib() {
    let n_ubatch = if total_mib >= 6 * 1024 { default } else { 128 };
    eprintln!("translate_srt: {total_mib} MiB total VRAM, n_ubatch={n_ubatch}");
    return n_ubatch;
  }

  default
}

/// Sequences to decode in parallel when the caller does not pick a number.
///
/// Every extra slot costs one sequence worth of KV cache but rides along on a forward pass the
/// engine has to run anyway, so the limit is memory rather than compute.
fn default_slots(use_gpu: bool) -> usize {
  if use_gpu {
    if let Some((_, total_mib)) = vram_mib() {
      return match total_mib {
        total if total >= 24 * 1024 => 32,
        total if total >= 12 * 1024 => 16,
        total if total >= 8 * 1024 => 8,
        _ => 4,
      };
    }

    // Metal reports no VRAM figure, but it shares system memory and the pool is demand-sized.
    return 32;
  }

  // CPU inference is compute bound rather than bandwidth bound, so wide batches pay off less.
  8
}

struct LoadedModel {
  backend: LlamaBackend,
  model: LlamaModel,
  n_ubatch: u32,
  use_gpu: bool,
}

fn engine_main(
  model_path: PathBuf,
  cpu: bool,
  verbose: bool,
  requested_slots: Option<usize>,
  ready: &std::sync::mpsc::Sender<Result<usize>>,
  jobs: mpsc::UnboundedReceiver<Job>,
) {
  let loaded = match load_model(model_path, cpu, verbose) {
    Ok(loaded) => loaded,
    Err(error) => {
      let _ = ready.send(Err(error));
      return;
    }
  };

  let slots = requested_slots.unwrap_or_else(|| default_slots(loaded.use_gpu));
  let context = match create_context(&loaded, slots) {
    Ok(context) => context,
    Err(error) => {
      let _ = ready.send(Err(error));
      return;
    }
  };

  eprintln!(
    "translate_srt: {slots} 路并发推理，KV 缓存 {} tokens，单个请求上限 {SLOT_CONTEXT_TOKENS} \
     tokens",
    context.n_ctx()
  );

  if ready.send(Ok(slots)).is_err() {
    return;
  }

  run_engine(&loaded.model, context, slots, jobs);
}

fn load_model(model_path: PathBuf, cpu: bool, verbose: bool) -> Result<LoadedModel> {
  if !verbose {
    send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
  }

  let backend = LlamaBackend::init()?;
  let use_gpu = !cpu && cfg!(any(feature = "cuda", feature = "metal", feature = "vulkan"));

  let (model, gpu_layers) = if use_gpu {
    let mut n_gpu = 9_999u32;
    let model = loop {
      let model = match LlamaModel::load_from_file(
        &backend,
        &model_path,
        &LlamaModelParams::default().with_n_gpu_layers(n_gpu),
      ) {
        Ok(model) => model,
        Err(_) => {
          let next = if n_gpu >= 9_999 { 64 } else { n_gpu / 2 };
          eprintln!("translate_srt: model load failed at {n_gpu} GPU layers, retrying with {next}");
          n_gpu = next;
          if n_gpu == 0 {
            return Err(anyhow!("无法加载 GGUF 模型，即使 GPU 层数已经降到 0"));
          }
          continue;
        }
      };

      let probe_ok = model
        .new_context(
          &backend,
          LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(8).expect("constant is non-zero")))
            .with_n_ubatch(1),
        )
        .ok()
        .and_then(|mut ctx| {
          let mut batch = LlamaBatch::new(8, 1);
          batch.add(LlamaToken(0), 0, &[0], true).ok()?;
          ctx.decode(&mut batch).ok()
        })
        .is_some();

      if probe_ok {
        break model;
      }

      let actual = model.n_layer() as u32;
      let current = n_gpu.min(actual);
      let next = current.saturating_sub((current / 10).max(1));
      eprintln!("translate_srt: GPU probe failed at {current} layers, retrying with {next}");
      n_gpu = next;
      drop(model);

      if n_gpu == 0 {
        return Err(anyhow!("GPU 推理失败，即使 GPU 层数已经降到 0"));
      }
    };

    let actual = model.n_layer() as u32;
    let on_gpu = n_gpu.min(actual);
    let gpu_layers = if on_gpu < actual { Some(on_gpu) } else { None };
    (model, gpu_layers)
  } else {
    let model = LlamaModel::load_from_file(
      &backend,
      model_path,
      &LlamaModelParams::default().with_n_gpu_layers(0),
    )
    .with_context(|| "无法加载 GGUF 模型")?;
    (model, None)
  };

  let n_ubatch = pick_n_ubatch(use_gpu);

  match (use_gpu, gpu_layers) {
    (false, _) => eprintln!("translate_srt: {} model layers, CPU only", model.n_layer()),
    (true, None) => eprintln!(
      "translate_srt: {} model layers, all offloaded to GPU",
      model.n_layer()
    ),
    (true, Some(n_gpu_layers)) => eprintln!(
      "translate_srt: {n_gpu_layers}/{} model layers on GPU, rest on CPU",
      model.n_layer()
    ),
  }

  Ok(LoadedModel {
    backend,
    model,
    n_ubatch,
    use_gpu,
  })
}

/// Builds the one context every request shares.
///
/// `kv_unified(false)` gives each sequence its own `n_ctx / n_seq_max` slice of the cache and its
/// own position space, which is what llama.cpp recommends when the sequences have no common prefix
/// worth sharing.
fn create_context(loaded: &LoadedModel, slots: usize) -> Result<LlamaContext<'_>> {
  let slots = u32::try_from(slots).context("并发推理路数超出范围")?;
  let prefix_sequences = u32::try_from(PREFIX_FORMATS.len()).expect("format count fits into a u32");
  // Headroom of one full-size request guarantees even the largest admissible prompt eventually
  // fits, however many slots are busy.
  let n_ctx = POOL_TOKENS_PER_SLOT
    .checked_mul(slots)
    .and_then(|total| total.checked_add(SLOT_CONTEXT_TOKENS + PREFIX_RESERVE_TOKENS))
    .context("并发推理路数过大，上下文超出范围")?;
  // A prompt that fits one slot must also fit a single batch alongside one token per active slot.
  let n_batch = SLOT_CONTEXT_TOKENS + slots;

  loaded
    .model
    .new_context(
      &loaded.backend,
      LlamaContextParams::default()
        .with_n_ctx(Some(
          NonZeroU32::new(n_ctx).expect("slot context size must be non-zero"),
        ))
        .with_n_batch(n_batch)
        .with_n_ubatch(loaded.n_ubatch.min(n_batch))
        .with_n_seq_max(slots + prefix_sequences)
        .with_kv_unified(true),
    )
    .with_context(|| "无法创建 llama.cpp 上下文")
}

/// A prompt prefix kept resident in its own llama.cpp sequence.
///
/// Every request repeats the same instructions before the subtitle text, and prefilling that
/// boilerplate dominated the forward passes. Decoding it once and having each slot reference the
/// same KV cells makes admission cost only the tokens that actually differ.
struct PromptPrefix {
  tokens: Vec<LlamaToken>,
  sequence: i32,
}

/// Decodes each format's shared prompt prefix into a reserved sequence.
///
/// The prefix is discovered by tokenising the same prompt around two different inputs and keeping
/// the tokens they agree on, so it never depends on how the tokeniser merges the boundary.
fn install_prefixes(
  model: &LlamaModel,
  context: &mut LlamaContext<'_>,
  batch: &mut LlamaBatch,
  slots: usize,
) -> Vec<Option<PromptPrefix>> {
  let mut prefixes = Vec::with_capacity(PREFIX_FORMATS.len());

  for (offset, format) in PREFIX_FORMATS.into_iter().enumerate() {
    prefixes.push(install_prefix(model, context, batch, slots, offset, format));
  }

  prefixes
}

fn install_prefix(
  model: &LlamaModel,
  context: &mut LlamaContext<'_>,
  batch: &mut LlamaBatch,
  slots: usize,
  offset: usize,
  format: TranslationFormat,
) -> Option<PromptPrefix> {
  let probe = |input: &str| {
    let prompt = TranslationPrompt::new(format).build(input);
    tokenize_prompt(model, &prompt.system, &prompt.user).ok()
  };

  let first = probe("A")?;
  let second = probe("B")?;
  let shared = common_prefix_len(&first, &second);
  if shared < MIN_SHARED_PREFIX_TOKENS || shared > PREFIX_RESERVE_TOKENS as usize {
    return None;
  }

  let tokens = first[.. shared].to_vec();
  let sequence = sequence_id(slots + offset);

  batch.clear();
  let last = i32::try_from(tokens.len() - 1).expect("prefix length fits into an i32");
  for (position, token) in (0_i32 ..).zip(tokens.iter()) {
    batch
      .add(*token, position, &[sequence], position == last)
      .ok()?;
  }

  if let Err(error) = context.decode(batch) {
    eprintln!("translate_srt: 无法预填充共享提示词前缀，将逐条完整处理: {error}");
    let _ = context.clear_kv_cache_seq(
      Some(u32::try_from(sequence).expect("sequence ids are non-negative")),
      None,
      None,
    );
    return None;
  }

  Some(PromptPrefix { tokens, sequence })
}

fn common_prefix_len(left: &[LlamaToken], right: &[LlamaToken]) -> usize {
  left
    .iter()
    .zip(right)
    .take_while(|(left, right)| left == right)
    .count()
}

fn format_index(format: TranslationFormat) -> usize {
  match format {
    TranslationFormat::Text => 0,
    TranslationFormat::Html => 1,
  }
}

/// One in-flight request occupying a llama.cpp sequence.
///
/// A slot's index in the engine's slot table doubles as its llama.cpp sequence id.
struct Slot {
  /// Position the next token is written to.
  pos: i32,
  max_new_tokens: usize,
  /// KV cells charged to the shared pool on admission and returned on release.
  reserved: usize,
  decoded_tokens: usize,
  output: String,
  decoder: Decoder,
  reply: oneshot::Sender<Result<String>>,
  /// Token sampled last step, waiting to be fed back in. `None` while the prompt is prefilling.
  next_token: Option<LlamaToken>,
  /// Set when generation aborted mid-stream; reported instead of the partial output.
  failure: Option<anyhow::Error>,
}

fn run_engine(
  model: &LlamaModel,
  mut context: LlamaContext<'_>,
  slots: usize,
  mut jobs: mpsc::UnboundedReceiver<Job>,
) {
  let slot_context = SLOT_CONTEXT_TOKENS as usize;
  let batch_capacity = slot_context + slots;
  let mut batch = LlamaBatch::new(batch_capacity, 1);
  let prefixes = install_prefixes(model, &mut context, &mut batch, slots);
  let resident = prefixes
    .iter()
    .flatten()
    .map(|prefix| prefix.tokens.len())
    .sum::<usize>();
  let mut pool_free = usize::try_from(context.n_ctx())
    .expect("context size fits into a usize")
    .saturating_sub(resident);
  let mut active: Vec<Option<Slot>> = (0 .. slots).map(|_| None).collect();
  let mut pending: VecDeque<Job> = VecDeque::new();
  let mut sampled_at: Vec<(usize, i32)> = Vec::with_capacity(slots);
  let mut finished: Vec<usize> = Vec::with_capacity(slots);
  let mut disconnected = false;
  let mut stats = EngineStats::default();

  loop {
    loop {
      match jobs.try_recv() {
        Ok(job) => pending.push_back(job),
        Err(mpsc::error::TryRecvError::Empty) => break,
        Err(mpsc::error::TryRecvError::Disconnected) => {
          disconnected = true;
          break;
        }
      }
    }

    if pending.is_empty() && active.iter().all(Option::is_none) {
      if disconnected {
        stats.report();
        return;
      }
      match jobs.blocking_recv() {
        Some(job) => pending.push_back(job),
        None => {
          stats.report();
          return;
        }
      }
      continue;
    }

    batch.clear();
    sampled_at.clear();
    let mut batch_tokens = 0i32;

    // Every sequence already generating contributes exactly one token to this forward pass.
    for (index, slot) in active.iter_mut().enumerate() {
      let Some(slot) = slot.as_mut() else { continue };
      let Some(token) = slot.next_token.take() else {
        continue;
      };

      batch
        .add(token, slot.pos, &[sequence_id(index)], true)
        .expect("the batch reserves room for one token per slot");
      sampled_at.push((index, batch_tokens));
      batch_tokens += 1;
      slot.pos += 1;
    }

    // Free slots pick up queued work, prefilling their whole prompt in the same forward pass.
    for (index, entry) in active.iter_mut().enumerate() {
      if entry.is_some() {
        continue;
      }
      let Some(job) = pending.pop_front() else {
        break;
      };

      let tokens = match tokenize_prompt(model, &job.system, &job.user) {
        Ok(tokens) if tokens.is_empty() => {
          let _ = job.reply.send(Err(anyhow!("提示词分词后为空")));
          continue;
        }
        Ok(tokens) => tokens,
        Err(error) => {
          let _ = job.reply.send(Err(error));
          continue;
        }
      };

      if tokens.len() >= slot_context {
        let _ = job.reply.send(Err(anyhow!(
          "提示词长度 {} tokens 超出单路上下文 {slot_context} tokens",
          tokens.len()
        )));
        continue;
      }

      // Reuse the resident boilerplate prefix when this prompt starts with it; only the tokens
      // that differ need a forward pass. One token must always be decoded to produce logits.
      let shared = prefixes
        .get(format_index(job.format))
        .and_then(Option::as_ref)
        .map_or(0, |prefix| common_prefix_len(&prefix.tokens, &tokens))
        .min(tokens.len() - 1);
      let shared = if shared >= MIN_SHARED_PREFIX_TOKENS {
        shared
      } else {
        0
      };

      let fresh = &tokens[shared ..];
      if usize::try_from(batch_tokens).expect("token counts are non-negative") + fresh.len()
        > batch_capacity
      {
        pending.push_front(job);
        break;
      }

      // Cells the shared prefix already holds are not charged again.
      let max_new_tokens =
        (MAX_NEW_TOKENS_PER_PROMPT_TOKEN * tokens.len()).min(slot_context - tokens.len());
      let reserved = fresh.len() + max_new_tokens;
      if reserved > pool_free {
        pending.push_front(job);
        break;
      }
      pool_free -= reserved;

      if shared > 0 {
        let prefix = prefixes[format_index(job.format)]
          .as_ref()
          .expect("a shared prefix length implies an installed prefix");
        let copied = context.kv_cache_seq_cp(
          prefix.sequence,
          sequence_id(index),
          None,
          Some(u32::try_from(shared).expect("prefix length fits into a u32")),
        );
        if let Err(error) = copied {
          let _ = job
            .reply
            .send(Err(anyhow!("无法复用共享提示词前缀: {error}")));
          continue;
        }
        stats.prefix_reused += shared;
      }

      let first = i32::try_from(shared).expect("prefix length fits into an i32");
      let prompt_tokens = i32::try_from(tokens.len()).expect("prompt length fits into an i32");
      let last = prompt_tokens - 1;
      for (position, token) in (first ..).zip(fresh.iter()) {
        batch
          .add(*token, position, &[sequence_id(index)], position == last)
          .expect("prompt length was checked against the remaining batch capacity");
      }

      sampled_at.push((
        index,
        batch_tokens + i32::try_from(fresh.len() - 1).expect("prompt length fits into an i32"),
      ));
      batch_tokens += i32::try_from(fresh.len()).expect("prompt length fits into an i32");

      *entry = Some(Slot {
        pos: prompt_tokens,
        max_new_tokens,
        reserved,
        decoded_tokens: 0,
        output: String::new(),
        decoder: UTF_8.new_decoder(),
        reply: job.reply,
        next_token: None,
        failure: None,
      });
    }

    if sampled_at.is_empty() {
      continue;
    }

    let step_started = std::time::Instant::now();
    if let Err(error) = context.decode(&mut batch) {
      for &(index, _) in &sampled_at {
        release_slot(
          &mut context,
          &mut active,
          &mut pool_free,
          index,
          Err(anyhow!("llama_decode() 失败: {error}")),
        );
      }
      continue;
    }

    finished.clear();
    let mut sample_time = std::time::Duration::ZERO;
    let mut first = true;
    for &(index, batch_index) in &sampled_at {
      let sample_started = std::time::Instant::now();
      let token = LlamaToken(argmax(context.get_logits_ith(batch_index)));
      if first {
        stats.first_sample += sample_started.elapsed();
        first = false;
      }
      sample_time += sample_started.elapsed();
      let Some(slot) = active[index].as_mut() else {
        continue;
      };
      slot.decoded_tokens += 1;

      if model.is_eog_token(token) {
        finished.push(index);
        continue;
      }

      match model.token_to_piece(token, &mut slot.decoder, true, None) {
        Ok(piece) => slot.output.push_str(&piece),
        Err(error) => {
          finished.push(index);
          slot.failure = Some(anyhow!("无法解码模型输出: {error}"));
          continue;
        }
      }

      if slot.decoded_tokens >= slot.max_new_tokens {
        finished.push(index);
      } else {
        slot.next_token = Some(token);
      }
    }

    let step_tokens = usize::try_from(batch_tokens).expect("token counts are non-negative");
    stats.record_step(
      sampled_at.len(),
      step_tokens,
      step_started.elapsed(),
      sample_time,
    );
    if step_tokens > sampled_at.len() {
      stats.prefill_steps += 1;
      stats.prefill_tokens += step_tokens - sampled_at.len();
      stats.prefill_busy += step_started.elapsed();
    }

    for &index in &finished {
      let outcome = match active[index].as_mut().and_then(|slot| slot.failure.take()) {
        Some(error) => Err(error),
        None => match active[index].as_ref() {
          Some(slot) => finish_output(&slot.output),
          None => continue,
        },
      };
      release_slot(&mut context, &mut active, &mut pool_free, index, outcome);
    }
  }
}

/// Throughput accounting for the engine loop, reported once when the engine shuts down.
#[derive(Default)]
struct EngineStats {
  steps: usize,
  /// Sum of sequences advanced per step; divided by `steps` this is the average batch width.
  sampled: usize,
  batch_tokens: usize,
  busy: std::time::Duration,
  sampling: std::time::Duration,
  first_sample: std::time::Duration,
  prefill_steps: usize,
  prefill_tokens: usize,
  prefill_busy: std::time::Duration,
  prefix_reused: usize,
}

impl EngineStats {
  fn record_step(
    &mut self,
    sampled: usize,
    batch_tokens: usize,
    elapsed: std::time::Duration,
    sampling: std::time::Duration,
  ) {
    self.steps += 1;
    self.sampled += sampled;
    self.batch_tokens += batch_tokens;
    self.busy += elapsed;
    self.sampling += sampling;
  }

  fn report(&self) {
    if self.steps == 0 {
      return;
    }

    let steps = self.steps as f64;
    eprintln!(
      "translate_srt: 推理 {} 步，平均并发 {:.2} 路，采样 {} 个 token，前向 {:.1}s（其中 argmax \
       {:.1}s，首个 {:.1}s），{:.1} tokens/s",
      self.steps,
      self.sampled as f64 / steps,
      self.sampled,
      self.busy.as_secs_f64(),
      self.sampling.as_secs_f64(),
      self.first_sample.as_secs_f64(),
      self.sampled as f64 / self.busy.as_secs_f64().max(f64::EPSILON),
    );
    eprintln!(
      "translate_srt: 其中 {} 步含预填充，共 {} 个提示词 token（另有 {} 个 token \
       命中共享前缀缓存），耗时 {:.1}s",
      self.prefill_steps,
      self.prefill_tokens,
      self.prefix_reused,
      self.prefill_busy.as_secs_f64(),
    );
  }
}

/// llama.cpp sequence id for a slot; a slot's table index is its sequence.
fn sequence_id(index: usize) -> i32 {
  i32::try_from(index).expect("slot count fits into an i32")
}

/// Answers the caller, drops the slot's KV cache, and makes the slot available again.
fn release_slot(
  context: &mut LlamaContext<'_>,
  active: &mut [Option<Slot>],
  pool_free: &mut usize,
  index: usize,
  outcome: Result<String>,
) {
  let Some(slot) = active[index].take() else {
    return;
  };

  let sequence = u32::try_from(index).expect("slot count fits into a u32");
  let _ = context.clear_kv_cache_seq(Some(sequence), None, None);
  *pool_free += slot.reserved;
  let _ = slot.reply.send(outcome);
}

/// Greedy pick over the raw logits.
///
/// `llama_sampler_sample` materialises a `llama_token_data` entry for every token first, which
/// costs several milliseconds per token on Gemma 3's 262k vocabulary — more than the forward pass
/// itself. The configured sampler chain reduced to greedy anyway (temperature 0, penalties and DRY
/// disabled), so scanning the logits directly is both equivalent and far cheaper.
/// Carrying the index through the reduction serialises it, so the maximum is found first over
/// independent lanes the compiler can vectorise, then located in a second pass.
fn argmax(logits: &[f32]) -> i32 {
  const LANES: usize = 16;

  let mut lanes = [f32::NEG_INFINITY; LANES];
  let (chunks, tail) = logits.as_chunks::<LANES>();

  for chunk in chunks {
    for (lane, &logit) in lanes.iter_mut().zip(chunk) {
      if logit > *lane {
        *lane = logit;
      }
    }
  }

  let mut best_logit = f32::NEG_INFINITY;
  for &logit in lanes.iter().chain(tail) {
    if logit > best_logit {
      best_logit = logit;
    }
  }

  let best_index = logits
    .iter()
    .position(|&logit| logit == best_logit)
    .unwrap_or(0);

  i32::try_from(best_index).expect("vocabulary size fits into an i32")
}

fn tokenize_prompt(model: &LlamaModel, system: &str, user: &str) -> Result<Vec<LlamaToken>> {
  let messages = [
    LlamaChatMessage::new("user".to_owned(), format!("{system}\n\n{user}"))
      .context("无法构造聊天消息")?,
  ];

  let llm_input = match model
    .chat_template(None)
    .ok()
    .and_then(|template| model.apply_chat_template(&template, &messages, true).ok())
  {
    Some(input) => input,
    None => {
      format!("<start_of_turn>user\n{system}\n\n{user}<end_of_turn>\n<start_of_turn>model\n")
    }
  };

  model
    .str_to_token(&llm_input, AddBos::Always)
    .with_context(|| "无法对提示词分词")
}

fn finish_output(output: &str) -> Result<String> {
  let output = if let Some(position) = output.find("<channel|>") {
    output[position + "<channel|>".len() ..].to_owned()
  } else if let Some(rest) = output.strip_prefix("<|channel>thought") {
    rest.trim_start_matches(['\n', ' ']).to_owned()
  } else {
    output.to_owned()
  };

  let output = output.replace("<end_of_turn>", "");
  let output = output.trim().to_owned();
  if output.is_empty() {
    return Err(anyhow!("模型返回了空文本"));
  }

  Ok(output)
}

struct TranslationPrompt {
  format: TranslationFormat,
}

impl TranslationPrompt {
  fn new(format: TranslationFormat) -> Self {
    Self { format }
  }

  fn build(&self, input: &str) -> BuiltPrompt {
    match self.format {
      TranslationFormat::Text => BuiltPrompt {
        system: concat!(
          "You are an expert subtitle translator. Translate English into Simplified Chinese. ",
          "Preserve the original meaning and tone. Return only the translated text. ",
          "Do not explain your work."
        )
        .to_owned(),
        user: format!(
          "Translate the subtitle line below from English to Simplified Chinese. Return only the \
           translated text.\n\nText: {input}\n\nTranslation:\n"
        ),
      },
      TranslationFormat::Html => BuiltPrompt {
        system: concat!(
          "You are an expert subtitle translator. Translate English into Simplified Chinese. ",
          "Preserve every HTML tag, attribute, paragraph id, and document order exactly as given. ",
          "Translate only the visible text content inside the HTML. Return only the translated \
           HTML fragment."
        )
        .to_owned(),
        user: format!(
          concat!(
            "Translate the visible text in the HTML fragment below from English to Simplified \
             Chinese. ",
            "Do not add or remove tags, attributes, ids, or wrappers. Return only the translated \
             HTML fragment.\n\n",
            "HTML:\n{}\n\nTranslated HTML:\n"
          ),
          input
        ),
      },
    }
  }
}

struct BuiltPrompt {
  system: String,
  user: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn argmax_picks_the_highest_logit() {
    assert_eq!(argmax(&[0.1, -2.0, 3.5, 3.4]), 2);
    assert_eq!(argmax(&[-1.0]), 0);
  }

  #[test]
  fn argmax_keeps_the_first_of_equal_logits() {
    assert_eq!(argmax(&[1.0, 1.0, 0.5]), 0);
  }

  #[test]
  fn finish_output_strips_channel_markers_and_turn_endings() {
    assert_eq!(
      finish_output("prefix<channel|>你好<end_of_turn>").unwrap(),
      "你好"
    );
    assert_eq!(finish_output("<|channel>thought\n 你好").unwrap(), "你好");
  }

  #[test]
  fn finish_output_rejects_empty_generations() {
    assert!(finish_output("   ").is_err());
  }
}
