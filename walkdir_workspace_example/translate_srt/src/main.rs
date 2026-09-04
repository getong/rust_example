use std::{
  fmt::Write as _,
  future::Future,
  num::NonZeroUsize,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use clap::{ArgAction, Parser};
use futures_util::{StreamExt, stream};
use many_cpus::SystemHardware;
mod local_llm;
use local_llm::LocalTranslator;
use scraper::{Html, Selector};
use tokio::{fs, sync::Semaphore, task::JoinSet, time::sleep};
use walkdir::WalkDir;

const DEFAULT_REQUEST_DELAY_MS: u64 = 0;
const DEFAULT_BATCH_SIZE: usize = 20;
const LIBRETRANSLATE_MAX_CHARS: usize = 5_000;
const LOCAL_MODEL_CONCURRENCY: usize = 1;
const MAX_TRANSLATION_ATTEMPTS: usize = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 250;

#[derive(Debug, Parser)]
#[command(
  version,
  about = "Translate English lines in SRT files and write bilingual subtitles"
)]
struct Args {
  /// Directory to scan recursively; defaults to the current working directory.
  #[arg(value_name = "DIRECTORY")]
  directory: Option<PathBuf>,

  /// Maximum concurrent files; defaults to max(2, half the available CPUs).
  #[arg(short, long)]
  jobs: Option<NonZeroUsize>,

  /// Path to the local GGUF translation model.
  #[arg(long, value_name = "FILE")]
  model_file: PathBuf,

  /// Force CPU inference even when GPU features are enabled.
  #[arg(long, action = ArgAction::SetTrue)]
  cpu: bool,

  /// Print verbose llama.cpp logs.
  #[arg(short, long, action = ArgAction::SetTrue)]
  verbose: bool,

  /// Delay after each translation request, in milliseconds.
  #[arg(long, default_value_t = DEFAULT_REQUEST_DELAY_MS, value_name = "MILLISECONDS")]
  request_delay_ms: u64,

  /// Maximum subtitle lines combined into a single translation request; defaults to 20.
  #[arg(long, value_name = "LINES")]
  batch_size: Option<NonZeroUsize>,
}

#[derive(Debug)]
struct FileOutcome {
  path: PathBuf,
  translated_lines: usize,
}

#[derive(Debug)]
struct RequestLimiter {
  requests: Semaphore,
  concurrency: usize,
  delay: Duration,
}

impl RequestLimiter {
  fn new(concurrency: NonZeroUsize, delay: Duration) -> Self {
    Self {
      requests: Semaphore::new(concurrency.get()),
      concurrency: concurrency.get(),
      delay,
    }
  }

  fn concurrency(&self) -> usize {
    self.concurrency
  }

  async fn run<F, T>(&self, request: F) -> Result<T>
  where
    F: Future<Output = T>,
  {
    let _permit = self
      .requests
      .acquire()
      .await
      .context("翻译请求限流器已关闭")?;
    let result = request.await;
    if !self.delay.is_zero() {
      sleep(self.delay).await;
    }
    Ok(result)
  }
}

#[derive(Clone, Copy, Debug)]
enum TranslationFormat {
  Text,
  Html,
}

#[tokio::main]
async fn main() {
  if let Err(error) = run().await {
    eprintln!("错误: {error:#}");
    std::process::exit(1);
  }
}

async fn run() -> Result<()> {
  let args = Args::parse();
  let jobs = args.jobs.map_or_else(default_job_count, NonZeroUsize::get);
  let batch_size = args
    .batch_size
    .map_or(DEFAULT_BATCH_SIZE, NonZeroUsize::get);
  let request_delay = Duration::from_millis(args.request_delay_ms);
  ensure!(
    args
      .model_file
      .extension()
      .and_then(|extension| extension.to_str())
      .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf")),
    "模型文件必须是 .gguf: {}",
    args.model_file.display()
  );
  let model_file = fs::canonicalize(&args.model_file)
    .await
    .with_context(|| format!("无法访问模型文件 {}", args.model_file.display()))?;
  let directory = match args.directory {
    Some(directory) => directory,
    None => std::env::current_dir().context("无法获取当前工作目录")?,
  };
  let directory = fs::canonicalize(&directory)
    .await
    .with_context(|| format!("无法访问目录 {}", directory.display()))?;

  ensure!(directory.is_dir(), "路径不是目录: {}", directory.display());

  let scan_root = directory.clone();
  let files = tokio::task::spawn_blocking(move || find_srt_files(&scan_root))
    .await
    .context("SRT 扫描任务意外终止")??;

  if files.is_empty() {
    println!("未在 {} 中找到 SRT 文件", directory.display());
    return Ok(());
  }

  println!(
    "找到 {} 个 SRT 文件，文件并发数: {jobs}，GGUF 模型: {}，推理设备: {}，请求间隔: \
     {}ms，批量翻译行数: {batch_size}",
    files.len(),
    model_file.display(),
    if args.cpu {
      "CPU"
    } else {
      "自动选择 GPU/CPU"
    },
    args.request_delay_ms
  );

  let client = Arc::new(LocalTranslator::new(model_file, args.cpu, args.verbose)?);
  let request_limiter = Arc::new(RequestLimiter::new(
    NonZeroUsize::new(LOCAL_MODEL_CONCURRENCY).expect("constant is non-zero"),
    request_delay,
  ));
  let mut paths = files.into_iter();
  let mut tasks = JoinSet::new();

  for path in paths.by_ref().take(jobs) {
    spawn_file_task(
      &mut tasks,
      Arc::clone(&client),
      Arc::clone(&request_limiter),
      batch_size,
      path,
    );
  }

  let mut failed_files = 0usize;
  while let Some(task_result) = tasks.join_next().await {
    match task_result {
      Ok(Ok(outcome)) if outcome.translated_lines == 0 => {
        println!("无需更新 {}", outcome.path.display());
      }
      Ok(Ok(outcome)) => {
        println!(
          "已更新 {}（新增 {} 行中文）",
          outcome.path.display(),
          outcome.translated_lines
        );
      }
      Ok(Err(error)) => {
        failed_files += 1;
        eprintln!("处理失败: {error:#}");
      }
      Err(error) => {
        failed_files += 1;
        eprintln!("处理任务意外终止: {error}");
      }
    }

    if let Some(path) = paths.next() {
      spawn_file_task(
        &mut tasks,
        Arc::clone(&client),
        Arc::clone(&request_limiter),
        batch_size,
        path,
      );
    }
  }

  if failed_files > 0 {
    bail!("{failed_files} 个 SRT 文件处理失败");
  }

  Ok(())
}

fn default_job_count() -> usize {
  let cpu_count = SystemHardware::current().processors().len();
  default_job_count_for_cpu_count(cpu_count)
}

fn default_job_count_for_cpu_count(cpu_count: usize) -> usize {
  (cpu_count / 2).max(2)
}

fn find_srt_files(root: &Path) -> Result<Vec<PathBuf>> {
  let mut files = Vec::new();

  for entry in WalkDir::new(root).follow_links(false) {
    let entry = entry.with_context(|| format!("扫描 {} 时出错", root.display()))?;
    if entry.file_type().is_file()
      && entry
        .path()
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("srt"))
    {
      files.push(entry.into_path());
    }
  }

  files.sort_unstable();
  Ok(files)
}

fn spawn_file_task(
  tasks: &mut JoinSet<Result<FileOutcome>>,
  client: Arc<LocalTranslator>,
  request_limiter: Arc<RequestLimiter>,
  batch_size: usize,
  path: PathBuf,
) {
  tasks.spawn(async move { translate_file(&client, &request_limiter, batch_size, path).await });
}

async fn translate_file(
  client: &LocalTranslator,
  request_limiter: &RequestLimiter,
  batch_size: usize,
  path: PathBuf,
) -> Result<FileOutcome> {
  let contents = fs::read_to_string(&path)
    .await
    .with_context(|| format!("无法以 UTF-8 读取 {}", path.display()))?;
  let lines: Vec<&str> = contents.lines().collect();
  let targets = translation_targets(&lines);

  if targets.is_empty() {
    return Ok(FileOutcome {
      path,
      translated_lines: 0,
    });
  }

  let mut translations: Vec<Option<String>> = (0 .. lines.len()).map(|_| None).collect();
  let mut skipped_lines = 0usize;
  let chunks = targets.chunks(batch_size).map(<[usize]>::to_vec);
  let chunk_results = stream::iter(chunks)
    .map(|chunk| {
      let texts = chunk.iter().map(|&index| lines[index]).collect::<Vec<_>>();
      async move {
        (
          chunk,
          translate_batch(client, request_limiter, &texts).await,
        )
      }
    })
    .buffer_unordered(request_limiter.concurrency())
    .collect::<Vec<_>>()
    .await;

  for (chunk, translated) in chunk_results {
    let translated =
      translated.with_context(|| format!("翻译 {} 第 {} 行失败", path.display(), chunk[0] + 1))?;
    for (&index, translation) in chunk.iter().zip(translated) {
      match translation {
        Some(text) => translations[index] = Some(text),
        None => skipped_lines += 1,
      }
    }
  }

  if skipped_lines > 0 {
    eprintln!(
      "{} 跳过了 {skipped_lines} 行无法翻译为中文的内容，已保留原文并继续处理其余行",
      path.display()
    );
  }

  let newline = if contents.contains("\r\n") {
    "\r\n"
  } else {
    "\n"
  };
  let output = render_lines(&lines, &translations, newline, contents.ends_with('\n'));

  fs::write(&path, output)
    .await
    .with_context(|| format!("无法写入 {}", path.display()))?;

  Ok(FileOutcome {
    path,
    translated_lines: targets.len() - skipped_lines,
  })
}

/// Translates fragments individually and batches only lines that look like standalone sentences.
///
/// Batch responses use numbered HTML paragraphs. Missing or ambiguously duplicated entries
/// are retried, while a wholly unusable response is split into smaller batches.
async fn translate_batch(
  client: &LocalTranslator,
  request_limiter: &RequestLimiter,
  texts: &[&str],
) -> Result<Vec<Option<String>>> {
  if texts.is_empty() {
    return Ok(Vec::new());
  }

  let mut translations: Vec<Option<String>> = (0 .. texts.len()).map(|_| None).collect();
  let mut skipped = vec![false; texts.len()];
  let mut batchable = Vec::with_capacity(texts.len());

  for (index, text) in texts.iter().enumerate() {
    if is_standalone_sentence(text) {
      batchable.push(index);
    } else {
      match translate_line(client, request_limiter, text).await? {
        Some(translated) => translations[index] = Some(translated),
        None => skipped[index] = true,
      }
    }
  }

  if batchable.is_empty() {
    return finish_batch_translations(translations, &skipped);
  }

  let mut pending_groups = Vec::with_capacity(1);
  pending_groups.push(batchable);

  while let Some(mut group) = pending_groups.pop() {
    if group.len() == 1 {
      let index = group[0];
      match translate_line(client, request_limiter, texts[index]).await? {
        Some(translated) => translations[index] = Some(translated),
        None => skipped[index] = true,
      }
      continue;
    }

    let group_texts = group.iter().map(|&index| texts[index]).collect::<Vec<_>>();
    let request_html = build_batch_html(&group_texts);
    if request_html.chars().count() >= LIBRETRANSLATE_MAX_CHARS {
      let middle = group.len() / 2;
      let right = group.split_off(middle);
      eprintln!(
        "批量翻译请求超过 LibreTranslate 的字符限制（共 {} 行），拆分为 {middle} 行和 {} 行",
        group.len() + right.len(),
        right.len()
      );
      pending_groups.push(right);
      pending_groups.push(group);
      continue;
    }
    let response_html = send_translation_request_with_retry(
      client,
      request_limiter,
      &request_html,
      TranslationFormat::Html,
      "批量翻译请求",
    )
    .await?;

    match parse_batch_html(&response_html, group.len()) {
      Ok(mut batch) => {
        let duplicate_count = discard_ambiguous_duplicate_translations(&group_texts, &mut batch);
        if duplicate_count > 0 {
          eprintln!("批量翻译检测到 {duplicate_count} 行不同原文共享相同译文，仅重试这些行");
        }

        let mut missing = Vec::new();
        for (local_index, translation) in batch.into_iter().enumerate() {
          let source_index = group[local_index];
          match translation {
            Some(text) => translations[source_index] = Some(text),
            None => missing.push(source_index),
          }
        }

        if missing.is_empty() {
          continue;
        }

        let translated_count = group.len() - missing.len();
        if translated_count > 0 {
          eprintln!(
            "批量翻译已还原 {translated_count}/{} 行，仅重试缺失的 {} 行",
            group.len(),
            missing.len()
          );
          pending_groups.push(missing);
          continue;
        }

        let middle = missing.len() / 2;
        let right = missing.split_off(middle);
        eprintln!(
          "批量翻译未还原任何段落（共 {} 行），拆分为 {middle} 行和 {} 行重试",
          group.len(),
          right.len()
        );
        pending_groups.push(right);
        pending_groups.push(missing);
      }
      Err(error) => {
        let middle = group.len() / 2;
        let right = group.split_off(middle);
        eprintln!(
          "批量翻译响应格式无效（共 {} 行）：{error:#}；拆分为 {middle} 行和 {} 行重试",
          group.len() + right.len(),
          right.len()
        );
        pending_groups.push(right);
        pending_groups.push(group);
      }
    }
  }

  finish_batch_translations(translations, &skipped)
}

/// Converts collected batch results into the caller's expected shape.
///
/// Lines marked `skipped` are deliberately given up on (e.g. the translation service keeps
/// echoing the original text back) and resolve to `None` rather than an error, so the rest of
/// the batch — and the file it belongs to — is still written out.
fn finish_batch_translations(
  translations: Vec<Option<String>>,
  skipped: &[bool],
) -> Result<Vec<Option<String>>> {
  translations
    .into_iter()
    .enumerate()
    .map(|(index, translation)| {
      if skipped[index] {
        Ok(None)
      } else {
        translation
          .map(Some)
          .with_context(|| format!("批量翻译缺少第 {} 行", index + 1))
      }
    })
    .collect()
}

/// Translates a single line, returning `Ok(None)` when the service demonstrably cannot produce
/// a Chinese translation (e.g. non-speech markers like "[BLANK_AUDIO]" that come back unchanged)
/// so the caller can skip that line instead of failing the whole file.
async fn translate_line(
  client: &LocalTranslator,
  request_limiter: &RequestLimiter,
  text: &str,
) -> Result<Option<String>> {
  let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

  for attempt in 1 ..= MAX_TRANSLATION_ATTEMPTS {
    let translated = send_translation_request_with_retry(
      client,
      request_limiter,
      text,
      TranslationFormat::Text,
      "翻译请求",
    )
    .await?;
    let translated = normalize_translation(&translated);

    if is_usable_translation(&translated) {
      return Ok(Some(translated));
    }

    if translated == text.trim() {
      eprintln!("翻译服务原样返回了原文，跳过该行且保留原文: {text:?}");
      return Ok(None);
    }

    if attempt < MAX_TRANSLATION_ATTEMPTS {
      eprintln!(
        "单行翻译结果不含中文（第 {attempt}/{MAX_TRANSLATION_ATTEMPTS} 次），{}ms 后重试",
        retry_delay.as_millis()
      );
      sleep(retry_delay).await;
      retry_delay = retry_delay.saturating_mul(2);
      continue;
    }

    eprintln!(
      "翻译服务连续 {MAX_TRANSLATION_ATTEMPTS} 次返回不含中文的结果，跳过该行且保留原文: {text:?} \
       -> {translated:?}"
    );
    return Ok(None);
  }

  bail!("翻译请求未执行")
}

async fn send_translation_request_with_retry(
  client: &LocalTranslator,
  request_limiter: &RequestLimiter,
  text: &str,
  format: TranslationFormat,
  request_name: &str,
) -> Result<String> {
  let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

  for attempt in 1 ..= MAX_TRANSLATION_ATTEMPTS {
    let result = request_limiter
      .run(send_translation_request(client, text, format))
      .await?;

    match result {
      Ok(translated) => return Ok(translated),
      Err(error) if attempt < MAX_TRANSLATION_ATTEMPTS => {
        eprintln!(
          "{request_name}失败（第 {attempt}/{MAX_TRANSLATION_ATTEMPTS} 次），{}ms 后重试: \
           {error:#}",
          retry_delay.as_millis()
        );
        sleep(retry_delay).await;
        retry_delay = retry_delay.saturating_mul(2);
      }
      Err(error) => return Err(error),
    }
  }

  bail!("翻译请求未执行")
}

async fn send_translation_request(
  client: &LocalTranslator,
  text: &str,
  format: TranslationFormat,
) -> Result<String> {
  ensure!(
    text.chars().count() < LIBRETRANSLATE_MAX_CHARS,
    "单次翻译内容超过字符限制（{} 字符）",
    LIBRETRANSLATE_MAX_CHARS
  );
  let translated = client
    .translate(text, format)
    .await
    .context("本地 GGUF 翻译失败")?
    .replace("\r\n", "\n")
    .replace('\r', "\n")
    .trim()
    .to_owned();
  ensure!(!translated.is_empty(), "翻译服务返回了空文本");

  if matches!(format, TranslationFormat::Text) {
    Ok(improve_formatting(text, &translated))
  } else {
    Ok(translated)
  }
}

fn improve_formatting(source: &str, translation: &str) -> String {
  let mut result = translation.trim().to_owned();
  if source.is_empty() || result.is_empty() {
    return result;
  }

  let source_last = source.chars().next_back();
  let result_last = result.chars().next_back();
  const PUNCTUATION: [char; 6] = ['!', '?', '.', ',', ';', '。'];

  match (source_last, result_last) {
    (Some(source_last), Some(result_last)) if PUNCTUATION.contains(&source_last) => {
      if source_last != result_last {
        if PUNCTUATION.contains(&result_last) {
          result.pop();
        }
        result.push(source_last);
      }
    }
    (_, Some(result_last)) if PUNCTUATION.contains(&result_last) => {
      result.pop();
    }
    _ => {}
  }

  if source.chars().all(|character| character.is_lowercase()) {
    result = result.to_lowercase();
  }
  if source.chars().all(|character| character.is_uppercase()) {
    result = result.to_uppercase();
  }

  if let (Some(source_first), Some(result_first)) = (source.chars().next(), result.chars().next()) {
    if source_first.is_lowercase() && result_first.is_uppercase() {
      result.replace_range(
        0 .. result_first.len_utf8(),
        &result_first.to_lowercase().to_string(),
      );
    } else if source_first.is_uppercase() && result_first.is_lowercase() {
      result.replace_range(
        0 .. result_first.len_utf8(),
        &result_first.to_uppercase().to_string(),
      );
    }
  }

  result.trim().to_owned()
}

fn build_batch_html(texts: &[&str]) -> String {
  let text_bytes = texts.iter().map(|text| text.len()).sum::<usize>();
  let mut html = String::with_capacity(text_bytes + texts.len() * 32 + 28);
  html.push_str("<div id=\"srt-batch\">");

  for (index, text) in texts.iter().enumerate() {
    write!(html, "<p id=\"srt-{index}\">").expect("writing to a String must succeed");
    push_escaped_html_text(&mut html, text);
    html.push_str("</p>");
  }

  html.push_str("</div>");
  html
}

fn push_escaped_html_text(output: &mut String, text: &str) {
  let mut unescaped_start = 0usize;

  for (index, character) in text.char_indices() {
    let entity = match character {
      '&' => "&amp;",
      '<' => "&lt;",
      '>' => "&gt;",
      _ => continue,
    };
    output.push_str(&text[unescaped_start .. index]);
    output.push_str(entity);
    unescaped_start = index + character.len_utf8();
  }

  output.push_str(&text[unescaped_start ..]);
}

fn parse_batch_html(html: &str, expected_lines: usize) -> Result<Vec<Option<String>>> {
  let document = Html::parse_fragment(html);
  let paragraph_selector =
    Selector::parse("p[id]").expect("the constant paragraph selector must be valid");
  let mut translations: Vec<Option<String>> = (0 .. expected_lines).map(|_| None).collect();

  for paragraph in document.select(&paragraph_selector) {
    let Some(index_text) = paragraph
      .value()
      .attr("id")
      .and_then(|id| id.strip_prefix("srt-"))
    else {
      continue;
    };
    let index = index_text
      .parse::<usize>()
      .with_context(|| format!("批量翻译返回了无效的段落编号 {index_text:?}"))?;
    ensure!(
      index < expected_lines,
      "批量翻译返回了越界的段落编号 {index}（期望 0-{}）",
      expected_lines.saturating_sub(1)
    );
    ensure!(
      translations[index].is_none(),
      "批量翻译重复返回了第 {index} 段"
    );

    let translated = normalize_translation(&paragraph.text().collect::<String>());
    if is_usable_translation(&translated) {
      translations[index] = Some(translated);
    }
  }

  Ok(translations)
}

fn discard_ambiguous_duplicate_translations(
  source_texts: &[&str],
  translations: &mut [Option<String>],
) -> usize {
  debug_assert_eq!(source_texts.len(), translations.len());

  let mut ambiguous = vec![false; translations.len()];
  for left in 0 .. translations.len() {
    for right in left + 1 .. translations.len() {
      if source_texts[left].trim() != source_texts[right].trim()
        && translations[left].is_some()
        && translations[left] == translations[right]
      {
        ambiguous[left] = true;
        ambiguous[right] = true;
      }
    }
  }

  let mut discarded = 0usize;
  for (translation, is_ambiguous) in translations.iter_mut().zip(ambiguous) {
    if is_ambiguous {
      *translation = None;
      discarded += 1;
    }
  }

  discarded
}

fn normalize_translation(text: &str) -> String {
  let mut normalized = text
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join(" ");

  let redundant_ascii_punctuation = match normalized.chars().last() {
    Some('.') => Some('。'),
    Some('!') => Some('！'),
    Some('?') => Some('？'),
    _ => None,
  };
  if redundant_ascii_punctuation.is_some_and(|full_width| {
    normalized
      .chars()
      .rev()
      .nth(1)
      .is_some_and(|previous| previous == full_width)
  }) {
    normalized.pop();
  }

  normalized
}

fn is_usable_translation(text: &str) -> bool {
  contains_visible_cjk(text)
}

fn is_standalone_sentence(text: &str) -> bool {
  let mut first_ascii_letter = None;
  let mut ends_with_terminal_punctuation = false;
  let mut inside_angle_tag = false;
  let mut brace_depth = 0usize;

  for character in text.chars() {
    match character {
      '<' if brace_depth == 0 => inside_angle_tag = true,
      '>' if inside_angle_tag => inside_angle_tag = false,
      '{' if !inside_angle_tag => brace_depth += 1,
      '}' if brace_depth > 0 => brace_depth -= 1,
      _ if inside_angle_tag || brace_depth > 0 || character.is_whitespace() => {}
      _ => {
        if first_ascii_letter.is_none() && character.is_ascii_alphabetic() {
          first_ascii_letter = Some(character);
        }
        match character {
          '.' | '!' | '?' => ends_with_terminal_punctuation = true,
          '\'' | '"' | ')' | ']' if ends_with_terminal_punctuation => {}
          _ => ends_with_terminal_punctuation = false,
        }
      }
    }
  }

  first_ascii_letter.is_some_and(|character| character.is_ascii_uppercase())
    && ends_with_terminal_punctuation
}

fn translation_targets(lines: &[&str]) -> Vec<usize> {
  let mut targets = Vec::new();
  let mut block_start = 0usize;

  for block_end in 0 ..= lines.len() {
    if block_end == lines.len() || lines[block_end].trim().is_empty() {
      add_block_translation_targets(lines, block_start, block_end, &mut targets);
      block_start = block_end + 1;
    }
  }

  targets
}

fn add_block_translation_targets(
  lines: &[&str],
  block_start: usize,
  block_end: usize,
  targets: &mut Vec<usize>,
) {
  let block = &lines[block_start .. block_end];
  let Some(text_start) = block.iter().position(|line| line.contains("-->")) else {
    return;
  };
  let text_start = text_start + 1;
  let text_lines = &block[text_start ..];

  if text_lines.iter().any(|line| contains_visible_cjk(line)) {
    return;
  }

  targets.extend(text_lines.iter().enumerate().filter_map(|(offset, line)| {
    is_translatable_line(line).then_some(block_start + text_start + offset)
  }));
}

fn is_translatable_line(line: &str) -> bool {
  !line.contains("-->") && visible_text_matches(line, |character| character.is_ascii_alphabetic())
}

fn contains_visible_cjk(line: &str) -> bool {
  visible_text_matches(line, |character| {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
    )
  })
}

fn visible_text_matches(line: &str, predicate: impl Fn(char) -> bool) -> bool {
  let mut inside_angle_tag = false;
  let mut brace_depth = 0usize;

  for character in line.chars() {
    match character {
      '<' if brace_depth == 0 => inside_angle_tag = true,
      '>' if inside_angle_tag => inside_angle_tag = false,
      '{' if !inside_angle_tag => brace_depth += 1,
      '}' if brace_depth > 0 => brace_depth -= 1,
      _ if !inside_angle_tag && brace_depth == 0 && predicate(character) => return true,
      _ => {}
    }
  }

  false
}

fn render_lines(
  lines: &[&str],
  translations: &[Option<String>],
  newline: &str,
  had_trailing_newline: bool,
) -> String {
  debug_assert_eq!(lines.len(), translations.len());

  let mut output = String::new();
  for (index, (line, translation)) in lines.iter().zip(translations).enumerate() {
    if index > 0 {
      output.push_str(newline);
    }
    output.push_str(line);

    if let Some(translation) = translation {
      output.push_str(newline);
      output.push_str(translation);
    }
  }

  if had_trailing_newline {
    output.push_str(newline);
  }

  output
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_job_count_is_at_least_two_and_otherwise_half_the_cpu_count() {
    assert_eq!(default_job_count_for_cpu_count(1), 2);
    assert_eq!(default_job_count_for_cpu_count(2), 2);
    assert_eq!(default_job_count_for_cpu_count(4), 2);
    assert_eq!(default_job_count_for_cpu_count(8), 4);
    assert_eq!(default_job_count_for_cpu_count(9), 4);
  }

  #[test]
  fn jobs_cli_argument_is_optional_and_accepts_an_explicit_value() {
    let default_args = Args::try_parse_from(["translate_srt", "--model-file", "model.gguf"])
      .expect("parsing arguments without --jobs should succeed");
    let explicit_args = Args::try_parse_from([
      "translate_srt",
      "--model-file",
      "model.gguf",
      "--jobs",
      "7",
      "--cpu",
      "--verbose",
      "--request-delay-ms",
      "25",
      "--batch-size",
      "30",
    ])
    .expect("parsing explicit local model options should succeed");

    assert_eq!(default_args.jobs, None);
    assert_eq!(default_args.model_file, PathBuf::from("model.gguf"));
    assert!(!default_args.cpu);
    assert!(!default_args.verbose);
    assert_eq!(default_args.request_delay_ms, DEFAULT_REQUEST_DELAY_MS);
    assert_eq!(default_args.batch_size, None);
    assert_eq!(explicit_args.jobs.map(NonZeroUsize::get), Some(7));
    assert!(explicit_args.cpu);
    assert!(explicit_args.verbose);
    assert_eq!(explicit_args.request_delay_ms, 25);
    assert_eq!(explicit_args.batch_size.map(NonZeroUsize::get), Some(30));
  }

  #[test]
  fn model_file_cli_argument_is_required() {
    let error = Args::try_parse_from(["translate_srt"])
      .expect_err("running without --model-file should be rejected");

    assert_eq!(
      error.kind(),
      clap::error::ErrorKind::MissingRequiredArgument
    );
  }

  #[tokio::test]
  async fn request_limiter_allows_only_the_configured_parallelism() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn observe_parallelism(active: &AtomicUsize, maximum: &AtomicUsize) {
      let current = active.fetch_add(1, Ordering::SeqCst) + 1;
      maximum.fetch_max(current, Ordering::SeqCst);
      sleep(Duration::from_millis(20)).await;
      active.fetch_sub(1, Ordering::SeqCst);
    }

    let limiter = RequestLimiter::new(NonZeroUsize::new(2).unwrap(), Duration::ZERO);
    let active = AtomicUsize::new(0);
    let maximum = AtomicUsize::new(0);

    let (first, second, third) = tokio::join!(
      limiter.run(observe_parallelism(&active, &maximum)),
      limiter.run(observe_parallelism(&active, &maximum)),
      limiter.run(observe_parallelism(&active, &maximum)),
    );

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(third.is_ok());
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
  }

  #[tokio::test]
  async fn request_limiter_waits_after_parallel_requests() {
    let limiter = RequestLimiter::new(NonZeroUsize::new(2).unwrap(), Duration::from_millis(20));
    let started_at = tokio::time::Instant::now();

    let (first, second) = tokio::join!(limiter.run(async {}), limiter.run(async {}));

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(started_at.elapsed() >= Duration::from_millis(20));
  }

  #[test]
  fn model_file_validation_requires_runtime_check() {
    let args = Args::try_parse_from(["translate_srt", "--model-file", "model.bin"])
      .expect("CLI parsing should defer file validation to runtime");

    assert_eq!(args.model_file, PathBuf::from("model.bin"));
  }

  #[test]
  fn improve_formatting_keeps_source_punctuation_style() {
    assert_eq!(improve_formatting("Hello!", "你好。"), "你好!");
    assert_eq!(improve_formatting("hello", "你好"), "你好");
  }

  #[test]
  fn translation_targets_selects_only_untranslated_english_text() {
    let lines = [
      "1",
      "00:00:01,000 --> 00:00:02,000",
      "<i>Hello world!</i>",
      "",
      "2",
      "00:00:03,000 --> 00:00:04,000",
      "Already translated",
      "已经翻译",
    ];

    assert_eq!(translation_targets(&lines), vec![2]);
  }

  #[test]
  fn translation_targets_skips_a_multiline_cue_that_already_has_chinese() {
    let lines = [
      "1",
      "00:00:01,000 --> 00:00:02,000",
      "First English line",
      "Second English line",
      "第一行中文",
      "第二行中文",
      "",
      "2",
      "00:00:03,000 --> 00:00:04,000",
      "Needs translation",
    ];

    assert_eq!(translation_targets(&lines), vec![9]);
  }

  #[test]
  fn translation_targets_skips_chinese_that_appears_before_english() {
    let lines = [
      "1",
      "00:00:01,000 --> 00:00:02,000",
      "已经翻译",
      "Already translated",
    ];

    assert!(translation_targets(&lines).is_empty());
  }

  #[test]
  fn formatting_tags_do_not_count_as_english_text() {
    assert!(!is_translatable_line("<font color=\"red\">你好</font>"));
    assert!(!is_translatable_line("{\\an8}你好"));
    assert!(is_translatable_line("<i>Hello</i>"));
  }

  #[test]
  fn normalization_removes_only_equivalent_trailing_ascii_punctuation() {
    assert_eq!(normalize_translation("你好吗？?"), "你好吗？");
    assert_eq!(normalize_translation("完成！!"), "完成！");
    assert_eq!(normalize_translation("结束。."), "结束。");
    assert_eq!(normalize_translation("真的吗?!"), "真的吗?!");
  }

  #[test]
  fn render_lines_preserves_crlf_and_trailing_newline() {
    let lines = ["1", "00:00:01,000 --> 00:00:02,000", "Hello"];
    let translations = [None, None, Some("你好".to_owned())];

    let output = render_lines(&lines, &translations, "\r\n", true);

    assert_eq!(
      output,
      "1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\n你好\r\n"
    );
  }

  #[test]
  fn batch_html_escapes_text_and_assigns_stable_paragraph_ids() {
    let html = build_batch_html(&["<i>Hello & welcome</i>", "Use x > 1"]);

    assert_eq!(
      html,
      "<div id=\"srt-batch\"><p id=\"srt-0\">&lt;i&gt;Hello &amp; welcome&lt;/i&gt;</p><p \
       id=\"srt-1\">Use x &gt; 1</p></div>"
    );
  }

  #[test]
  fn batch_html_parser_recovers_compact_out_of_order_paragraphs() {
    let html = concat!(
      "<div id=\"srt-batch\"><p id=\"srt-1\">第二 <i>行</i></p>",
      "<p id=\"srt-0\">第一 &amp; 一半</p></div>"
    );

    let translations = parse_batch_html(html, 2).expect("valid batch HTML should parse");

    assert_eq!(
      translations,
      [Some("第一 & 一半".to_owned()), Some("第二 行".to_owned())]
    );
  }

  #[test]
  fn batch_html_parser_preserves_valid_paragraphs_when_some_are_missing() {
    let translations = parse_batch_html("<p id=\"srt-0\">第一行</p><p id=\"srt-1\"></p>", 3)
      .expect("missing paragraphs should remain available for targeted retries");

    assert_eq!(translations, [Some("第一行".to_owned()), None, None]);
  }

  #[test]
  fn batch_html_parser_retries_empty_and_punctuation_only_translations() {
    let translations = parse_batch_html(
      "<p id=\"srt-0\"></p><p id=\"srt-1\">。</p><p id=\"srt-2\">有效译文</p>",
      3,
    )
    .expect("unusable paragraphs should remain available for targeted retries");

    assert_eq!(translations, [None, None, Some("有效译文".to_owned())]);
  }

  #[test]
  fn finish_batch_translations_turns_skipped_lines_into_none_instead_of_an_error() {
    let translations = vec![Some("已译文".to_owned()), None, None];
    let skipped = [false, true, false];

    let error = finish_batch_translations(translations.clone(), &skipped)
      .expect_err("an unskipped missing translation must still be reported as an error");
    assert!(error.to_string().contains("第 3 行"));

    let skipped_all_missing = [false, true, true];
    let resolved = finish_batch_translations(translations, &skipped_all_missing)
      .expect("skipped lines should resolve to None rather than failing the batch");
    assert_eq!(resolved, [Some("已译文".to_owned()), None, None]);
  }

  #[test]
  fn only_standalone_sentences_are_safe_to_batch() {
    assert!(is_standalone_sentence("What makes an agent?"));
    assert!(is_standalone_sentence("<i>Agents can act.</i>"));
    assert!(is_standalone_sentence("{\\an8}\"Agents can act!\""));
    assert!(!is_standalone_sentence(" and tasks."));
    assert!(!is_standalone_sentence("Agents can act across"));
  }

  #[test]
  fn duplicate_translations_for_different_sources_are_discarded() {
    let sources = ["First source.", "Second source.", "Repeated source."];
    let duplicate = "错误地合并了其他字幕。".to_owned();
    let mut translations = [
      Some(duplicate.clone()),
      Some("独立译文。".to_owned()),
      Some(duplicate),
    ];

    let discarded = discard_ambiguous_duplicate_translations(&sources, &mut translations);

    assert_eq!(discarded, 2);
    assert_eq!(translations, [None, Some("独立译文。".to_owned()), None]);
  }

  #[test]
  fn duplicate_translations_for_repeated_sources_are_allowed() {
    let sources = ["Okay.", "Okay."];
    let mut translations = [Some("好的。".to_owned()), Some("好的。".to_owned())];

    let discarded = discard_ambiguous_duplicate_translations(&sources, &mut translations);

    assert_eq!(discarded, 0);
    assert!(translations.iter().all(Option::is_some));
  }
}
