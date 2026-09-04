use std::{num::NonZeroU32, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use encoding_rs::UTF_8;
use llama_cpp_2::{
  LogOptions,
  context::{LlamaContext, params::LlamaContextParams},
  llama_backend::LlamaBackend,
  llama_batch::LlamaBatch,
  model::{AddBos, LlamaChatMessage, LlamaModel, params::LlamaModelParams},
  sampling::LlamaSampler,
  send_logs_to_tracing,
  token::LlamaToken,
};
use parking_lot::Mutex;

use crate::TranslationFormat;

#[derive(Debug, thiserror::Error)]
pub enum LocalLlmError {
  #[error("本地 GGUF 模型忙碌，请稍后重试")]
  Busy,
}

#[derive(Clone, Debug)]
pub struct LocalTranslator {
  llm: Arc<LocalLlm>,
}

impl LocalTranslator {
  pub fn new(model_path: PathBuf, cpu: bool, verbose: bool) -> Result<Self> {
    Ok(Self {
      llm: Arc::new(LocalLlm::new(model_path, cpu, verbose)?),
    })
  }

  pub async fn translate(&self, input: &str, format: TranslationFormat) -> Result<String> {
    let prompt = TranslationPrompt::new(format).build(input);
    let llm = Arc::clone(&self.llm);

    tokio::task::spawn_blocking(move || llm.run_prompt(prompt.system, prompt.user))
      .await
      .context("本地 GGUF 推理任务意外终止")?
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
  if use_gpu {
    if let Some((_, total_mib)) = vram_mib() {
      let n_ubatch = if total_mib >= 6 * 1024 { default } else { 128 };
      eprintln!("translate_srt: {total_mib} MiB total VRAM, n_ubatch={n_ubatch}");
      return n_ubatch;
    }
  }

  default
}

#[derive(Debug)]
struct LocalLlm {
  backend: LlamaBackend,
  model: LlamaModel,
  prompt_lock: Mutex<()>,
  n_ubatch: u32,
}

struct LocalLlmContext<'a> {
  llm: &'a LocalLlm,
  ctx: LlamaContext<'a>,
  ctx_size: i32,
}

impl LocalLlm {
  fn new(model_path: PathBuf, cpu: bool, verbose: bool) -> Result<Self> {
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
            eprintln!(
              "translate_srt: model load failed at {n_gpu} GPU layers, retrying with {next}"
            );
            n_gpu = next;
            if n_gpu == 0 {
              return Err(anyhow::anyhow!(
                "无法加载 GGUF 模型，即使 GPU 层数已经降到 0"
              ));
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
          return Err(anyhow::anyhow!("GPU 推理失败，即使 GPU 层数已经降到 0"));
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

    Ok(Self {
      backend,
      model,
      prompt_lock: Mutex::new(()),
      n_ubatch,
    })
  }

  fn create_context(&self, ctx_size: i32) -> Result<LocalLlmContext<'_>> {
    let ctx = self
      .model
      .new_context(
        &self.backend,
        LlamaContextParams::default()
          .with_n_ctx(Some(
            NonZeroU32::new(ctx_size as u32).expect("prompt context size must be non-zero"),
          ))
          .with_n_ubatch(self.n_ubatch),
      )
      .with_context(|| "无法创建 llama.cpp 上下文")?;

    Ok(LocalLlmContext {
      llm: self,
      ctx,
      ctx_size,
    })
  }

  fn run_prompt(&self, system: String, user: String) -> Result<String> {
    let messages = [
      LlamaChatMessage::new("user".to_owned(), format!("{system}\n\n{user}"))
        .context("无法构造聊天消息")?,
    ];

    let llm_input = match self.model.chat_template(None).ok().and_then(|template| {
      self
        .model
        .apply_chat_template(&template, &messages, true)
        .ok()
    }) {
      Some(input) => input,
      None => {
        eprintln!("translate_srt: apply_chat_template failed, using Gemma fallback format");
        format!("<start_of_turn>user\n{system}\n\n{user}<end_of_turn>\n<start_of_turn>model\n")
      }
    };

    let tokens = self
      .model
      .str_to_token(&llm_input, AddBos::Always)
      .with_context(|| "无法对提示词分词")?;
    let ctx_size = (tokens.len() as i32) * 3;

    let _lock = self
      .prompt_lock
      .try_lock_for(Duration::from_secs(120))
      .ok_or(LocalLlmError::Busy)?;
    let mut ctx = self.create_context(ctx_size)?;
    ctx.process(tokens)
  }
}

impl LocalLlmContext<'_> {
  fn process(&mut self, tokens: Vec<LlamaToken>) -> Result<String> {
    let mut batch = LlamaBatch::new(self.ctx_size.try_into()?, 1);
    let last_index = (tokens.len() - 1) as i32;

    for (index, token) in (0_i32 ..).zip(tokens.into_iter()) {
      batch.add(token, index, &[0], index == last_index)?;
    }

    self
      .ctx
      .decode(&mut batch)
      .with_context(|| "llama_decode() failed")?;

    let mut current_token = batch.n_tokens();
    let mut decoder = UTF_8.new_decoder();
    let seq_breakers = vec![b"\n", b":", b"\"", b"*"];
    let mut sampler = LlamaSampler::chain_simple([
      LlamaSampler::penalties(self.llm.model.n_vocab(), 64, 1.0, 0.0, 0.0),
      LlamaSampler::dry(&self.llm.model, 0.0, 1.75, 2, -1, seq_breakers),
      LlamaSampler::top_k(40),
      LlamaSampler::typical(1.0, 0),
      LlamaSampler::top_p(0.95, 0),
      LlamaSampler::min_p(0.05, 0),
      LlamaSampler::xtc(0.0, 0.1, 0, 42),
      LlamaSampler::temp_ext(0.0, 0.0, 1.0),
      LlamaSampler::dist(42),
    ]);
    let mut output = String::new();

    while current_token <= self.ctx_size {
      let token = sampler.sample(&self.ctx, batch.n_tokens() - 1);
      sampler.accept(token);

      if self.llm.model.is_eog_token(token) {
        break;
      }

      let piece = self
        .llm
        .model
        .token_to_piece(token, &mut decoder, true, None)?;
      output.push_str(&piece);

      batch.clear();
      batch.add(token, current_token, &[0], true)?;
      current_token += 1;

      self
        .ctx
        .decode(&mut batch)
        .with_context(|| "模型继续推理失败")?;
    }

    let output = if let Some(position) = output.find("<channel|>") {
      output[position + "<channel|>".len() ..].to_owned()
    } else if let Some(rest) = output.strip_prefix("<|channel>thought") {
      rest.trim_start_matches(['\n', ' ']).to_owned()
    } else {
      output
    };

    let output = output.replace("<end_of_turn>", "");
    let output = output.trim().to_owned();
    if output.is_empty() {
      return Err(anyhow::anyhow!("模型返回了空文本"));
    }

    Ok(output)
  }
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
