use std::{
  future::Future,
  num::NonZeroUsize,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use many_cpus::SystemHardware;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::Semaphore, task::JoinSet, time::sleep};
use walkdir::WalkDir;

const DEFAULT_ENDPOINT: &str = "http://0.0.0.0:5050/translate";
const DEFAULT_REQUEST_DELAY_MS: u64 = 0;
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

  /// LibreTranslate-compatible HTTP endpoint.
  #[arg(long, default_value = DEFAULT_ENDPOINT)]
  endpoint: String,

  /// Delay after each translation request, in milliseconds.
  #[arg(long, default_value_t = DEFAULT_REQUEST_DELAY_MS, value_name = "MILLISECONDS")]
  request_delay_ms: u64,
}

#[derive(Debug)]
struct FileOutcome {
  path: PathBuf,
  translated_lines: usize,
}

#[derive(Debug)]
struct RequestLimiter {
  single_request: Semaphore,
  delay: Duration,
}

impl RequestLimiter {
  fn new(delay: Duration) -> Self {
    Self {
      single_request: Semaphore::new(1),
      delay,
    }
  }

  async fn run<F, T>(&self, request: F) -> Result<T>
  where
    F: Future<Output = T>,
  {
    let _permit = self
      .single_request
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

#[derive(Serialize)]
struct TranslationRequest<'a> {
  q: &'a str,
  source: &'static str,
  target: &'static str,
  format: &'static str,
  api_key: &'static str,
}

#[derive(Deserialize)]
struct TranslationResponse {
  #[serde(rename = "translatedText")]
  translated_text: Option<String>,
  error: Option<String>,
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
  let request_delay = Duration::from_millis(args.request_delay_ms);
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
    "找到 {} 个 SRT 文件，并发处理数: {jobs}，请求间隔: {}ms",
    files.len(),
    args.request_delay_ms
  );

  let client = Client::builder()
    .timeout(Duration::from_secs(60))
    .build()
    .context("无法创建 HTTP 客户端")?;
  let endpoint: Arc<str> = Arc::from(args.endpoint);
  let request_limiter = Arc::new(RequestLimiter::new(request_delay));
  let mut paths = files.into_iter();
  let mut tasks = JoinSet::new();

  for path in paths.by_ref().take(jobs) {
    spawn_file_task(
      &mut tasks,
      client.clone(),
      Arc::clone(&endpoint),
      Arc::clone(&request_limiter),
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
        client.clone(),
        Arc::clone(&endpoint),
        Arc::clone(&request_limiter),
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
  client: Client,
  endpoint: Arc<str>,
  request_limiter: Arc<RequestLimiter>,
  path: PathBuf,
) {
  tasks.spawn(async move { translate_file(&client, &endpoint, &request_limiter, path).await });
}

async fn translate_file(
  client: &Client,
  endpoint: &str,
  request_limiter: &RequestLimiter,
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
  for index in targets.iter().copied() {
    let translated = translate_line(client, endpoint, request_limiter, lines[index])
      .await
      .with_context(|| format!("翻译 {} 第 {} 行失败", path.display(), index + 1))?;
    translations[index] = Some(translated);
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
    translated_lines: targets.len(),
  })
}

async fn translate_line(
  client: &Client,
  endpoint: &str,
  request_limiter: &RequestLimiter,
  text: &str,
) -> Result<String> {
  let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

  for attempt in 1 ..= MAX_TRANSLATION_ATTEMPTS {
    let result = request_limiter
      .run(send_translation_request(client, endpoint, text))
      .await?;

    match result {
      Ok(translated) => return Ok(translated),
      Err(error) if attempt < MAX_TRANSLATION_ATTEMPTS => {
        eprintln!(
          "翻译请求失败（第 {attempt}/{MAX_TRANSLATION_ATTEMPTS} 次），{}ms 后重试: {error:#}",
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

async fn send_translation_request(client: &Client, endpoint: &str, text: &str) -> Result<String> {
  let request = TranslationRequest {
    q: text,
    source: "en",
    target: "zh-Hans",
    format: "text",
    api_key: "",
  };
  let response = client
    .post(endpoint)
    .json(&request)
    .send()
    .await
    .context("无法连接翻译服务")?;
  let status = response.status();

  if !status.is_success() {
    let body = response.text().await.context("无法读取翻译服务错误响应")?;
    bail!("翻译服务返回 HTTP {status}: {}", body.trim());
  }

  let payload: TranslationResponse = response.json().await.context("翻译服务返回了无效的 JSON")?;
  if let Some(error) = payload.error {
    bail!("翻译服务报错: {error}");
  }

  let translated = payload
    .translated_text
    .context("翻译响应缺少 translatedText 字段")?
    .trim()
    .replace(['\r', '\n'], " ");
  ensure!(!translated.is_empty(), "翻译服务返回了空文本");

  Ok(translated)
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
    let default_args = Args::try_parse_from(["translate_srt"])
      .expect("parsing arguments without --jobs should succeed");
    let explicit_args = Args::try_parse_from(["translate_srt", "--jobs", "7"])
      .expect("parsing an explicit --jobs value should succeed");

    assert_eq!(default_args.jobs, None);
    assert_eq!(default_args.request_delay_ms, DEFAULT_REQUEST_DELAY_MS);
    assert_eq!(explicit_args.jobs.map(NonZeroUsize::get), Some(7));
  }

  #[tokio::test]
  async fn request_limiter_serializes_requests_and_waits_after_each_one() {
    let limiter = RequestLimiter::new(Duration::from_millis(20));
    let started_at = tokio::time::Instant::now();

    let (first, second) = tokio::join!(limiter.run(async {}), limiter.run(async {}));

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(started_at.elapsed() >= Duration::from_millis(40));
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
  fn render_lines_preserves_crlf_and_trailing_newline() {
    let lines = ["1", "00:00:01,000 --> 00:00:02,000", "Hello"];
    let translations = [None, None, Some("你好".to_owned())];

    let output = render_lines(&lines, &translations, "\r\n", true);

    assert_eq!(
      output,
      "1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\n你好\r\n"
    );
  }
}
