use std::{fs, path::Path, time::SystemTime};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::document;

pub const SYSTEM_PROMPT: &str =
  "You are a UI designer. Generate a polished, responsive, accessible interface following the \
   user's brief, in Chinese unless otherwise requested. Output ONE complete offline HTML/CSS \
   document, from <!doctype html> to </html>, with literal <head>, inline <style>, and <body>. \
   Use inline CSS and simple inline SVG; no JavaScript, external resources, frames, embeds, \
   remote links, forms submitting data, or media playback. Native controls, details, and local \
   #anchor navigation can demonstrate states. Clearly label unavailable backend actions as \
   previews. Never claim real functionality that is not implemented. When modifying an existing \
   design, preserve its content and features unless the user asks otherwise. Return the ENTIRE \
   revised document, not a diff. No markdown or explanations. Keep output compact, close style \
   early, and always complete the document within the output budget.";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
  pub endpoint: String,
  pub model: String,
  pub max_tokens: u32,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      endpoint: "http://localhost:11434/v1/chat/completions".into(),
      model: "Maternion/mai-ui:8b".into(),
      max_tokens: 16384,
    }
  }
}

impl Settings {
  pub fn validate(&self) -> Result<()> {
    let url = reqwest::Url::parse(&self.endpoint).context("接口 URL 无效")?;
    ensure!(
      matches!(url.scheme(), "http" | "https"),
      "仅支持 HTTP(S) 接口"
    );
    ensure!(!self.model.trim().is_empty(), "模型名称不能为空");
    ensure!(
      (256 ..= 131072).contains(&self.max_tokens),
      "输出上限应为 256–131072"
    );
    Ok(())
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Revision {
  pub parent: Option<usize>,
  pub prompt: String,
  pub html: String,
  pub model: String,
  pub created_at: u64,
  pub output_tokens: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
  pub settings: Settings,
  pub draft: String,
  pub prompts: Vec<String>,
  pub revisions: Vec<Revision>,
  pub active: Option<usize>,
}

impl Default for Session {
  fn default() -> Self {
    Self {
      settings: Settings::default(),
      draft: "设计一个中文桌面视频播放器，深色主题，包含播放区域、播放列表、媒体库和字幕设置。\
              仅展示 UI 设计效果。"
        .into(),
      prompts: Vec::new(),
      revisions: Vec::new(),
      active: None,
    }
  }
}

impl Session {
  pub fn load(path: &Path) -> Result<Self> {
    let bytes = match fs::read(path) {
      Ok(bytes) => bytes,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
      Err(error) => return Err(error).context("读取会话失败"),
    };
    let session: Self = serde_json::from_slice(&bytes)
      .context("会话 JSON 损坏；请备份后修复或使用其他 MAI_UI_SESSION 路径")?;
    session.validate()?;
    Ok(session)
  }

  pub fn validate(&self) -> Result<()> {
    ensure!(
      self.active.is_none_or(|index| index < self.revisions.len()),
      "当前版本索引无效"
    );
    for (index, revision) in self.revisions.iter().enumerate() {
      ensure!(
        revision.parent.is_none_or(|parent| parent < index),
        "版本父节点无效"
      );
      ensure!(!revision.prompt.trim().is_empty(), "版本提示词为空");
      document::validate(&revision.html).context("保存的版本页面不完整")?;
    }
    Ok(())
  }

  pub fn save(&self, path: &Path) -> Result<()> {
    self.validate()?;
    if let Some(parent) = path
      .parent()
      .filter(|parent| !parent.as_os_str().is_empty())
    {
      fs::create_dir_all(parent).context("创建会话目录失败")?;
    }
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(self)?;
    let mut file = fs::File::create(&temporary).context("创建会话临时文件失败")?;
    use std::io::Write as _;
    file.write_all(&bytes).context("写入会话失败")?;
    file.sync_all().context("同步会话失败")?;
    drop(file);
    fs::rename(&temporary, path).context("替换会话失败；临时文件中保留了新数据")?;
    Ok(())
  }

  pub fn current(&self) -> Option<&Revision> {
    self.active.and_then(|index| self.revisions.get(index))
  }

  pub fn remember_prompt(&mut self, prompt: &str) {
    let prompt = prompt.trim();
    if !prompt.is_empty() && !self.prompts.iter().any(|saved| saved == prompt) {
      self.prompts.push(prompt.to_owned());
    }
  }

  pub fn commit(&mut self, prompt: String, html: String, output_tokens: Option<u64>) -> Result<()> {
    let html = document::validate(&html)?;
    ensure!(!prompt.trim().is_empty(), "提示词不能为空");
    self.remember_prompt(&prompt);
    self.revisions.push(Revision {
      parent: self.active,
      prompt,
      html,
      model: self.settings.model.clone(),
      created_at: SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs(),
      output_tokens,
    });
    self.active = Some(self.revisions.len() - 1);
    Ok(())
  }

  pub fn select(&mut self, index: Option<usize>) -> Result<()> {
    ensure!(
      index.is_none_or(|index| index < self.revisions.len()),
      "版本不存在"
    );
    self.active = index;
    Ok(())
  }

  pub fn back(&mut self) {
    self.active = self.current().and_then(|revision| revision.parent);
  }

  pub fn request(&self, prompt: &str) -> Result<Value> {
    self.validate()?;
    self.settings.validate()?;
    ensure!(!prompt.trim().is_empty(), "请先输入提示词");
    let mut ancestry = Vec::new();
    let mut cursor = self.active;
    while let Some(index) = cursor {
      let revision = &self.revisions[index];
      ancestry.push(revision);
      cursor = revision.parent;
    }
    let mut messages = vec![json!({"role": "system", "content": SYSTEM_PROMPT})];
    for revision in ancestry.into_iter().rev() {
      messages.push(json!({"role": "user", "content": revision.prompt}));
      messages.push(json!({"role": "assistant", "content": revision.html}));
    }
    messages.push(json!({"role": "user", "content": prompt.trim()}));
    Ok(json!({
        "model": self.settings.model,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
        "max_tokens": self.settings.max_tokens,
    }))
  }
}
