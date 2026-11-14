use std::{sync::mpsc, thread, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use tokio::sync::watch;

use crate::document::{self, MAX_DOCUMENT_BYTES};

pub enum Event {
  Connected,
  Delta {
    content: String,
    thinking_chars: usize,
  },
  Finished(Result<Output>),
}

pub struct Output {
  pub content: String,
  pub tokens: Option<u64>,
}

pub struct Job {
  pub events: mpsc::Receiver<Event>,
  cancel: watch::Sender<bool>,
}

impl Drop for Job {
  fn drop(&mut self) {
    let _ = self.cancel.send(true);
  }
}

pub fn start(endpoint: String, request: Value, api_key: Option<String>) -> Result<Job> {
  start_with_format(endpoint, request, api_key, ResponseFormat::Html)
}

#[derive(Clone, Copy, Default)]
pub enum ResponseFormat {
  #[default]
  Html,
  Grounding,
}

pub fn start_with_format(
  endpoint: String,
  request: Value,
  api_key: Option<String>,
  format: ResponseFormat,
) -> Result<Job> {
  let (events, receiver) = mpsc::channel();
  let (cancel, mut cancelled) = watch::channel(false);
  thread::Builder::new()
    .name("mai-ui-generation".into())
    .spawn(move || {
      let result = (|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
          .enable_all()
          .build()?;
        runtime.block_on(async {
          tokio::select! {
              _ = cancelled.changed() => bail!("生成已取消"),
              result = generate(&endpoint, request, api_key, &events, format) => result,
          }
        })
      })();
      let _ = events.send(Event::Finished(result));
    })
    .context("无法启动生成线程")?;
  Ok(Job {
    events: receiver,
    cancel,
  })
}

async fn generate(
  endpoint: &str,
  request: Value,
  api_key: Option<String>,
  events: &mpsc::Sender<Event>,
  format: ResponseFormat,
) -> Result<Output> {
  let client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(30))
    .timeout(Duration::from_secs(2 * 60 * 60))
    .build()?;
  let mut builder = client
    .post(endpoint)
    .header("Accept", "text/event-stream")
    .json(&request);
  if let Some(key) = api_key.filter(|key| !key.is_empty()) {
    builder = builder.bearer_auth(key);
  }
  let mut response = builder
    .send()
    .await
    .context("连接模型失败；请确认 Ollama 已启动、模型已安装和接口地址正确")?;
  let status = response.status();
  if !status.is_success() {
    let mut body = Vec::new();
    while body.len() < 8192 {
      let Some(chunk) = response.chunk().await? else {
        break;
      };
      let remaining = 8192 - body.len();
      body.extend_from_slice(&chunk[.. chunk.len().min(remaining)]);
    }
    bail!(
      "模型接口返回 HTTP {status}: {}",
      String::from_utf8_lossy(&body)
    );
  }
  let _ = events.send(Event::Connected);
  let mut decoder = StreamDecoder::new(format);
  while let Some(chunk) = response.chunk().await.context("读取模型流失败")? {
    for event in decoder.push(&chunk)? {
      let _ = events.send(event);
    }
    if decoder.done {
      break;
    }
  }
  decoder.finish()
}

#[derive(Default)]
pub struct StreamDecoder {
  pending: Vec<u8>,
  data: String,
  html: String,
  tokens: Option<u64>,
  finish_reason: Option<String>,
  done: bool,
  format: ResponseFormat,
}

impl StreamDecoder {
  pub fn new(format: ResponseFormat) -> Self {
    Self {
      format,
      ..Self::default()
    }
  }

  pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Event>> {
    ensure!(!self.done, "模型在流结束后继续发送数据");
    self.pending.extend_from_slice(bytes);
    let mut events = Vec::new();
    while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
      let line: Vec<_> = self.pending.drain(..= newline).collect();
      let line = std::str::from_utf8(&line)
        .context("模型流不是 UTF-8")?
        .trim_end_matches(['\r', '\n']);
      if line.is_empty() {
        self.dispatch(&mut events)?;
        if self.done {
          break;
        }
      } else if let Some(data) = line.strip_prefix("data:") {
        if !self.data.is_empty() {
          self.data.push('\n');
        }
        self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
      }
      ensure!(self.data.len() <= MAX_DOCUMENT_BYTES, "单个流事件过大");
    }
    ensure!(
      self.pending.len() <= MAX_DOCUMENT_BYTES,
      "模型流缺少事件分隔符或事件过大"
    );
    Ok(events)
  }

  fn dispatch(&mut self, events: &mut Vec<Event>) -> Result<()> {
    if self.data.is_empty() {
      return Ok(());
    }
    let data = std::mem::take(&mut self.data);
    if data.trim() == "[DONE]" {
      self.done = true;
      return Ok(());
    }
    let response: Value =
      serde_json::from_str(&data).context("无法解析 SSE JSON；需要 OpenAI 兼容的流式接口")?;
    if let Some(error) = response.get("error") {
      bail!("模型报告错误: {error}");
    }
    if let Some(tokens) = response
      .pointer("/usage/completion_tokens")
      .and_then(Value::as_u64)
    {
      self.tokens = Some(tokens);
    }
    if let Some(reason) = response
      .pointer("/choices/0/finish_reason")
      .and_then(Value::as_str)
    {
      self.finish_reason = Some(reason.to_owned());
    }
    let delta = &response["choices"][0]["delta"];
    let content = delta["content"].as_str().unwrap_or_default();
    let thinking = delta["reasoning_content"]
      .as_str()
      .or_else(|| delta["reasoning"].as_str())
      .unwrap_or_default();
    ensure!(
      self.html.len() + content.len() <= MAX_DOCUMENT_BYTES,
      "模型输出超过 4 MiB 限制"
    );
    self.html.push_str(content);
    if !content.is_empty() || !thinking.is_empty() {
      events.push(Event::Delta {
        content: content.to_owned(),
        thinking_chars: thinking.chars().count(),
      });
    }
    Ok(())
  }

  pub fn finish(mut self) -> Result<Output> {
    if !self.done && (!self.pending.is_empty() || !self.data.is_empty()) {
      self.push(b"\n\n")?;
    }
    ensure!(self.done, "模型流提前断开，未收到 [DONE]；当前版本未改变");
    ensure!(
      self.finish_reason.as_deref() == Some("stop"),
      "模型未正常完成（结束原因: {}）；如达到 token 上限，请精简提示词或提高上限后重试",
      self.finish_reason.as_deref().unwrap_or("未知")
    );
    let content = match self.format {
      ResponseFormat::Html => document::validate(&self.html)?,
      ResponseFormat::Grounding => {
        crate::grounding::parse_response(&self.html)?;
        self.html
      }
    };
    Ok(Output {
      content,
      tokens: self.tokens,
    })
  }
}
