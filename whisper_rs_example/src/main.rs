use std::{
  env,
  ffi::{CStr, CString},
  fs,
  io::{ErrorKind, Write},
  os::{
    raw::{c_char, c_int},
    unix::ffi::OsStrExt,
  },
  path::{Path, PathBuf},
  process::{Command, Output, Stdio},
  sync::{Arc, mpsc},
  thread,
};

use anyhow::{Context, Result, bail, ensure};
use many_cpus::SystemHardware;
use tempfile::Builder;
use walkdir::WalkDir;
use whisper_rs::whisper_rs_sys;

const DEFAULT_BACKUP_DIR: &str = "backup-20251212";
const DEFAULT_WHISPER_MODEL: &str = "~/test/cpp/whisper.cpp/models/ggml-base.en.bin";
const JOBS_ENV: &str = "MP4S_TO_SRT_JOBS";
const WHISPER_THREADS_ENV: &str = "WHISPER_THREADS";
const WHISPER_PROMPT_ENV: &str = "WHISPER_PROMPT";
const WHISPER_SAMPLE_RATE: u32 = 16_000;
const DEFAULT_MAX_WHISPER_THREADS_PER_TASK: usize = 4;
const COMMAND_ERROR_TAIL_LINES: usize = 8;
const WHISPER_CLI_BEAM_SIZE: c_int = 5;
const WHISPER_CLI_BEST_OF: c_int = 5;
const WHISPER_TEMPERATURE: f32 = 0.0;
const WHISPER_TEMPERATURE_INC: f32 = 0.2;
const WHISPER_ENTROPY_THOLD: f32 = 2.4;
const WHISPER_LOGPROB_THOLD: f32 = -1.0;
const WHISPER_NO_SPEECH_THOLD: f32 = 0.6;
const SUPPORTED_MEDIA_EXTENSIONS: &[&str] = &[
  "3g2", "3gp", "aac", "ac3", "aif", "aiff", "amr", "asf", "avi", "divx", "dv", "f4v", "flac",
  "flv", "m2ts", "m4a", "m4b", "m4v", "mka", "mkv", "mov", "mp2", "mp3", "mp4", "mpeg", "mpg",
  "mts", "mxf", "oga", "ogg", "ogm", "ogv", "opus", "ra", "rm", "rmvb", "ts", "vob", "wav", "webm",
  "wma", "wmv",
];

#[derive(Debug, Clone)]
struct Config {
  scan_dir: PathBuf,
  backup_dir: PathBuf,
  model: PathBuf,
  language: String,
  jobs: usize,
  whisper_threads: Option<usize>,
}

#[derive(Debug, Clone)]
struct Task {
  input: PathBuf,
  output: PathBuf,
}

#[derive(Debug)]
pub struct TaskResult {
  pub input: PathBuf,
  output: PathBuf,
  result: Result<TaskOutcome>,
}

#[derive(Debug)]
enum TaskOutcome {
  Generated,
  Skipped(String),
}

fn main() -> Result<()> {
  whisper_rs::install_logging_hooks();

  let config = Config::from_env()?;

  ensure_command_available("ffmpeg")?;
  ensure_command_available("ffprobe")?;
  ensure!(
    config.model.is_file(),
    "model file not found: {}",
    config.model.display()
  );

  let tasks = collect_tasks(&config)?;
  if tasks.is_empty() {
    eprintln!(
      "No supported media files need SRT generation under: {}",
      config.scan_dir.display()
    );
    return Ok(());
  }

  let jobs = config.jobs.min(tasks.len());
  let whisper_threads = config
    .whisper_threads
    .unwrap_or_else(|| default_whisper_threads(jobs));
  eprintln!(
    "Processing {} media file(s) with {jobs} worker thread(s), {whisper_threads} whisper thread(s)
      per worker",
    tasks.len()
  );

  run_tasks(tasks, config, jobs, whisper_threads)
}

impl Config {
  fn from_env() -> Result<Self> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(args.len() <= 1, "usage: whisper_rs_example [SCAN_DIR]");

    let scan_dir = match args.first() {
      Some(path) => PathBuf::from(path),
      None => env::current_dir().context("failed to read current directory")?,
    }
    .canonicalize()
    .context("failed to canonicalize scan directory")?;

    let backup_dir = backup_dir()?;
    let model_path = env::var_os("WHISPER_MODEL")
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from(DEFAULT_WHISPER_MODEL));
    let model = expand_home_path(model_path)?;
    let language = env::var("WHISPER_LANG").unwrap_or_else(|_| String::from("en"));
    let jobs = parallel_jobs()?;
    let whisper_threads = optional_positive_usize(WHISPER_THREADS_ENV)?;

    Ok(Self {
      scan_dir,
      backup_dir,
      model,
      language,
      jobs,
      whisper_threads,
    })
  }
}

fn backup_dir() -> Result<PathBuf> {
  if let Some(path) = env::var_os("MP4S_TO_SRT_BACKUP_DIR") {
    return expand_home_path(PathBuf::from(path));
  }

  let home = env::var_os("HOME").context("HOME is not set")?;
  Ok(PathBuf::from(home).join(DEFAULT_BACKUP_DIR))
}

fn expand_home_path(path: PathBuf) -> Result<PathBuf> {
  let Some(path_str) = path.to_str() else {
    return Ok(path);
  };

  if path_str == "~" {
    let home = env::var_os("HOME").context("HOME is not set")?;
    return Ok(PathBuf::from(home));
  }

  if let Some(path_without_home) = path_str.strip_prefix("~/") {
    let home = env::var_os("HOME").context("HOME is not set")?;
    return Ok(PathBuf::from(home).join(path_without_home));
  }

  Ok(path)
}

fn parallel_jobs() -> Result<usize> {
  if let Some(jobs) = env::var_os(JOBS_ENV) {
    let jobs = jobs
      .to_string_lossy()
      .parse::<usize>()
      .with_context(|| format!("{JOBS_ENV} must be a positive integer"))?;
    ensure!(jobs > 0, "{JOBS_ENV} must be a positive integer");
    return Ok(jobs);
  }

  Ok(SystemHardware::current().processors().len().max(1))
}

fn optional_positive_usize(env_name: &str) -> Result<Option<usize>> {
  let Some(value) = env::var_os(env_name) else {
    return Ok(None);
  };

  let value = value
    .to_string_lossy()
    .parse::<usize>()
    .with_context(|| format!("{env_name} must be a positive integer"))?;
  ensure!(value > 0, "{env_name} must be a positive integer");
  Ok(Some(value))
}

fn default_whisper_threads(worker_count: usize) -> usize {
  let cpu_count = SystemHardware::current().processors().len().max(1);
  let threads = (cpu_count / worker_count.max(1)).max(1);
  threads.min(DEFAULT_MAX_WHISPER_THREADS_PER_TASK)
}

fn collect_tasks(config: &Config) -> Result<Vec<Task>> {
  let mut tasks = Vec::new();

  for entry in WalkDir::new(&config.scan_dir)
    .follow_links(true)
    .into_iter()
  {
    let entry = entry.with_context(|| {
      format!(
        "failed to read an entry under {}",
        config.scan_dir.display()
      )
    })?;
    let path = entry.path();
    if !entry.file_type().is_file() || !is_supported_media(path) {
      continue;
    }

    if let Some(task) = task_for_input(path, config)? {
      tasks.push(task);
    }
  }

  Ok(tasks)
}

fn task_for_input(input: &Path, config: &Config) -> Result<Option<Task>> {
  if let Some(existing_subtitle) = existing_subtitle_for(input) {
    eprintln!(
      "Skipping existing subtitle: {}",
      existing_subtitle.display()
    );
    return Ok(None);
  }

  match has_audio_stream(input) {
    Ok(true) => {}
    Ok(false) => {
      eprintln!(
        "Skipping media file without an audio stream: {}",
        input.display()
      );
      return Ok(None);
    }
    Err(err) => {
      eprintln!(
        "Skipping media file with unreadable audio stream info: {}",
        input.display()
      );
      eprintln!("  {err:#}");
      return Ok(None);
    }
  }

  let input_dir = input
    .parent()
    .with_context(|| format!("failed to read parent directory for {}", input.display()))?;

  let output = if can_create_file_in(input_dir) {
    input.with_extension("srt")
  } else {
    let relative_input = input.strip_prefix(&config.scan_dir).unwrap_or(input);
    let backup_base = config.backup_dir.join(relative_input);
    if let Some(existing_subtitle) = existing_subtitle_for(&backup_base) {
      eprintln!(
        "Skipping existing subtitle: {}",
        existing_subtitle.display()
      );
      return Ok(None);
    }
    backup_base.with_extension("srt")
  };

  Ok(Some(Task {
    input: input.to_path_buf(),
    output,
  }))
}

fn existing_subtitle_for(input: &Path) -> Option<PathBuf> {
  ["srt", "vtt"]
    .into_iter()
    .map(|extension| input.with_extension(extension))
    .find(|subtitle| subtitle.is_file())
}

fn is_supported_media(path: &Path) -> bool {
  path
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| {
      SUPPORTED_MEDIA_EXTENSIONS
        .iter()
        .any(|supported_extension| extension.eq_ignore_ascii_case(supported_extension))
    })
}

fn can_create_file_in(dir: &Path) -> bool {
  let probe_path = dir.join(format!(".mp4s-to-srt-write-test.{}", std::process::id()));

  match fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&probe_path)
  {
    Ok(_) => {
      let _ = fs::remove_file(probe_path);
      true
    }
    Err(err) if err.kind() == ErrorKind::AlreadyExists => true,
    Err(_) => false,
  }
}

fn run_tasks(tasks: Vec<Task>, config: Config, jobs: usize, whisper_threads: usize) -> Result<()> {
  eprintln!("Loading whisper model: {}", config.model.display());
  let whisper_context = Arc::new(RawWhisperContext::new(&config.model)?);

  let (task_tx, task_rx) = mpsc::channel::<Task>();
  let (result_tx, result_rx) = mpsc::channel::<TaskResult>();

  let worker_count = jobs.max(1);
  let mut workers = Vec::with_capacity(worker_count);
  let task_rx = Arc::new(std::sync::Mutex::new(task_rx));

  for _ in 0 .. worker_count {
    let task_rx = Arc::clone(&task_rx);
    let result_tx = result_tx.clone();
    let config = config.clone();
    let whisper_context = Arc::clone(&whisper_context);

    workers.push(thread::spawn(move || {
      loop {
        let task = {
          let task_rx = match task_rx.lock() {
            Ok(task_rx) => task_rx,
            Err(_) => break,
          };
          task_rx.recv()
        };

        let task = match task {
          Ok(task) => task,
          Err(_) => break,
        };

        let result = process_task(&config, &whisper_context, &task, whisper_threads);
        let _ = result_tx.send(TaskResult {
          input: task.input,
          output: task.output,
          result,
        });
      }
    }));
  }

  drop(result_tx);

  for task in tasks {
    task_tx
      .send(task)
      .context("failed to send task to worker thread")?;
  }
  drop(task_tx);

  let mut failed = 0usize;
  for task_result in result_rx {
    match task_result.result {
      Ok(TaskOutcome::Generated) => eprintln!("Generated: {}", task_result.output.display()),
      Ok(TaskOutcome::Skipped(reason)) => {
        eprintln!("Skipped: {},   {reason}", task_result.input.display());
      }
      Err(err) => {
        failed += 1;
        eprintln!("Failed: {},  {err:#}", task_result.input.display());
      }
    }
  }

  for worker in workers {
    if worker.join().is_err() {
      failed += 1;
      eprintln!("worker thread panicked");
    }
  }

  ensure!(failed == 0, "{failed} file(s) failed");
  Ok(())
}

fn process_task(
  config: &Config,
  whisper_context: &RawWhisperContext,
  task: &Task,
  whisper_threads: usize,
) -> Result<TaskOutcome> {
  if let Some(parent) = task.output.parent() {
    fs::create_dir_all(parent)
      .with_context(|| format!("failed to create output directory {}", parent.display()))?;
  }

  let temp_dir = Builder::new()
    .prefix("whisper-mp4-to-srt.")
    .tempdir()
    .context("failed to create temp directory")?;
  let wav_file = Builder::new()
    .prefix("audio.")
    .suffix(".wav")
    .tempfile_in(temp_dir.path())
    .context("failed to create temp wav file")?;
  let wav_path = wav_file.path().to_path_buf();
  let generated_srt = temp_dir.path().join("output.srt");

  eprintln!("Extracting audio: {}", task.input.display());
  if let Err(err) = extract_audio(&task.input, &wav_path) {
    return Ok(TaskOutcome::Skipped(format!(
      "failed to extract audio with whisper.cpp script ffmpeg arguments: {err:#}"
    )));
  }
  if let Err(err) = ensure_complete_audio(&task.input, &wav_path) {
    return Ok(TaskOutcome::Skipped(format!(
      "extracted audio is incomplete, not generating a short SRT: {err:#}"
    )));
  }

  eprintln!("Transcribing to SRT: {}", task.output.display());
  transcribe_to_srt(
    config,
    whisper_context,
    &wav_path,
    &generated_srt,
    whisper_threads,
  )?;

  ensure!(
    generated_srt.is_file(),
    "whisper-rs did not produce an SRT file: {}",
    generated_srt.display()
  );
  move_generated_srt(&generated_srt, &task.output)?;

  Ok(TaskOutcome::Generated)
}

fn move_generated_srt(source: &Path, target: &Path) -> Result<()> {
  match fs::rename(source, target) {
    Ok(()) => Ok(()),
    Err(err) if is_cross_device_link(&err) => copy_generated_srt_across_filesystems(source, target),
    Err(err) => Err(err).with_context(|| {
      format!(
        "failed to move generated SRT from {} to {}",
        source.display(),
        target.display()
      )
    }),
  }
}

fn copy_generated_srt_across_filesystems(source: &Path, target: &Path) -> Result<()> {
  let target_dir = target
    .parent()
    .with_context(|| format!("failed to read target directory for {}", target.display()))?;
  let temp_file = Builder::new()
    .prefix(".whisper-mp4-to-srt.")
    .suffix(".srt")
    .tempfile_in(target_dir)
    .with_context(|| {
      format!(
        "failed to create temporary SRT file next to {}",
        target.display()
      )
    })?;

  fs::copy(source, temp_file.path()).with_context(|| {
    format!(
      "failed to copy generated SRT from {} to {}",
      source.display(),
      temp_file.path().display()
    )
  })?;

  temp_file
    .persist(target)
    .map(|_| ())
    .map_err(|err| err.error)
    .with_context(|| {
      format!(
        "failed to move copied SRT from temporary file to {}",
        target.display()
      )
    })
}

#[cfg(unix)]
fn is_cross_device_link(err: &std::io::Error) -> bool {
  err.raw_os_error() == Some(18)
}

#[cfg(not(unix))]
fn is_cross_device_link(_err: &std::io::Error) -> bool {
  false
}

fn extract_audio(input: &Path, wav_file: &Path) -> Result<()> {
  match extract_audio_whisper_cpp_script(input, wav_file) {
    Ok(()) => Ok(()),
    Err(err) => {
      let script_error = format!("{err:#}");
      eprintln!(
        "Script-compatible ffmpeg extraction failed, trying damaged-AAC channel fallback: {}",
        input.display()
      );
      extract_audio_damaged_aac_channel(input, wav_file).with_context(|| {
        format!("script-compatible ffmpeg extraction failed first:\n{script_error}")
      })
    }
  }
}

fn extract_audio_whisper_cpp_script(input: &Path, wav_file: &Path) -> Result<()> {
  // Keep this command in lockstep with whisper.cpp/scripts/mp4-to-srt.sh.
  let mut command = Command::new("ffmpeg");
  command
    .arg("-hide_banner")
    .arg("-loglevel")
    .arg("error")
    .arg("-y")
    .arg("-i")
    .arg(input)
    .arg("-vn")
    .arg("-ar")
    .arg(WHISPER_SAMPLE_RATE.to_string())
    .arg("-ac")
    .arg("1")
    .arg("-c:a")
    .arg("pcm_s16le")
    .arg(wav_file);

  run_command(&mut command, "ffmpeg")
}

fn extract_audio_damaged_aac_channel(input: &Path, wav_file: &Path) -> Result<()> {
  // Corrupt AAC frames can report impossible channel layouts, which breaks
  // ffmpeg's automatic mono remix. Pin the first channel and keep decoding.
  let mut command = Command::new("ffmpeg");
  command
    .arg("-hide_banner")
    .arg("-loglevel")
    .arg("fatal")
    .arg("-y")
    .arg("-max_error_rate")
    .arg("1")
    .arg("-i")
    .arg(input)
    .arg("-vn")
    .arg("-af")
    .arg(format!("pan=mono|c0=c0,aresample={WHISPER_SAMPLE_RATE}"))
    .arg("-c:a")
    .arg("pcm_s16le")
    .arg(wav_file);

  run_command(&mut command, "ffmpeg")
}

fn ensure_complete_audio(input: &Path, wav_file: &Path) -> Result<()> {
  let input_duration = audio_duration(input).or_else(|_| media_duration(input))?;
  let wav_duration = media_duration(wav_file)?;

  ensure!(
    wav_duration + 1.0 >= input_duration * 0.98,
    "extracted wav is incomplete: input audio {:.3}s, wav {:.3}s",
    input_duration,
    wav_duration
  );

  Ok(())
}

fn audio_duration(path: &Path) -> Result<f64> {
  ffprobe_duration(
    path,
    &["-select_streams", "a:0", "-show_entries", "stream=duration"],
  )
}

fn media_duration(path: &Path) -> Result<f64> {
  ffprobe_duration(path, &["-show_entries", "format=duration"])
}

fn has_audio_stream(path: &Path) -> Result<bool> {
  let output = Command::new("ffprobe")
    .arg("-hide_banner")
    .arg("-v")
    .arg("error")
    .arg("-select_streams")
    .arg("a:0")
    .arg("-show_entries")
    .arg("stream=index")
    .arg("-of")
    .arg("csv=p=0")
    .arg(path)
    .output()
    .with_context(|| format!("failed to run ffprobe for {}", path.display()))?;

  ensure_success_output(&output, "ffprobe")?;

  let stdout = String::from_utf8(output.stdout)
    .with_context(|| format!("ffprobe output was not UTF-8 for {}", path.display()))?;
  Ok(!stdout.trim().is_empty())
}

fn ffprobe_duration(path: &Path, args: &[&str]) -> Result<f64> {
  let mut command = Command::new("ffprobe");
  command.arg("-hide_banner").arg("-v").arg("error");
  for arg in args {
    command.arg(arg);
  }
  let output = command
    .arg("-of")
    .arg("default=noprint_wrappers=1:nokey=1")
    .arg(path)
    .output()
    .with_context(|| format!("failed to run ffprobe for {}", path.display()))?;

  ensure_success_output(&output, "ffprobe")?;

  let stdout = String::from_utf8(output.stdout)
    .with_context(|| format!("ffprobe output was not UTF-8 for {}", path.display()))?;
  let duration = stdout
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty() && *line != "N/A")
    .with_context(|| format!("ffprobe did not return a duration for {}", path.display()))?;

  duration
    .parse::<f64>()
    .with_context(|| format!("failed to parse ffprobe duration for {}", path.display()))
}

fn transcribe_to_srt(
  config: &Config,
  whisper_context: &RawWhisperContext,
  wav_file: &Path,
  output: &Path,
  whisper_threads: usize,
) -> Result<()> {
  let audio = read_wav_samples(wav_file)?;
  let mut state = whisper_context.create_state()?;
  let language = whisper_language(&config.language)?;
  let initial_prompt = whisper_initial_prompt()?;
  let whisper_threads =
    c_int::try_from(whisper_threads).context("WHISPER_THREADS is too large for whisper-rs")?;

  let params = whisper_cli_full_params(
    whisper_threads,
    language.as_deref(),
    initial_prompt.as_c_str(),
  );
  state
    .full(params, &audio)
    .with_context(|| format!("failed to transcribe {}", wav_file.display()))?;
  write_srt(&state, output)
}

struct RawWhisperContext {
  ctx: *mut whisper_rs_sys::whisper_context,
}

// SAFETY: whisper.cpp contexts are designed to be shared across threads when
// each transcription uses its own whisper_state. This wrapper never mutates the
// context directly after construction and every worker creates a fresh state.
unsafe impl Send for RawWhisperContext {}
// SAFETY: see the Send impl above. Shared access is limited to whisper.cpp calls
// that take the context plus a per-task state.
unsafe impl Sync for RawWhisperContext {}

impl RawWhisperContext {
  fn new(model: &Path) -> Result<Self> {
    let model = CString::new(model.as_os_str().as_bytes())
      .with_context(|| format!("model path contains a null byte: {}", model.display()))?;
    // SAFETY: returns a plain value and requires no preconditions.
    let params = unsafe { whisper_rs_sys::whisper_context_default_params() };
    // SAFETY: model is a live NUL-terminated path for the duration of the call.
    // params was produced by whisper.cpp. A null return is checked below.
    let ctx = unsafe {
      whisper_rs_sys::whisper_init_from_file_with_params_no_state(model.as_ptr(), params)
    };
    ensure!(!ctx.is_null(), "failed to load whisper model");
    Ok(Self { ctx })
  }

  fn create_state(&self) -> Result<RawWhisperState<'_>> {
    // SAFETY: self.ctx is a non-null context created by whisper.cpp and freed
    // only when RawWhisperContext is dropped, after all borrowed states.
    let state = unsafe { whisper_rs_sys::whisper_init_state(self.ctx) };
    ensure!(!state.is_null(), "failed to create whisper state");
    Ok(RawWhisperState {
      context: self,
      state,
    })
  }
}

impl Drop for RawWhisperContext {
  fn drop(&mut self) {
    // SAFETY: ctx was returned by whisper_init_from_file_with_params_no_state
    // and is freed exactly once here.
    unsafe {
      whisper_rs_sys::whisper_free(self.ctx);
    }
  }
}

struct RawWhisperState<'a> {
  context: &'a RawWhisperContext,
  state: *mut whisper_rs_sys::whisper_state,
}

impl RawWhisperState<'_> {
  fn full(&mut self, params: whisper_rs_sys::whisper_full_params, audio: &[f32]) -> Result<()> {
    ensure!(!audio.is_empty(), "wav did not contain any audio samples");
    let sample_count = c_int::try_from(audio.len()).context("audio is too long for whisper-rs")?;
    // SAFETY: context and state are live; params contains pointers to values
    // that outlive this synchronous call; audio points to sample_count f32s.
    let status = unsafe {
      whisper_rs_sys::whisper_full_with_state(
        self.context.ctx,
        self.state,
        params,
        audio.as_ptr(),
        sample_count,
      )
    };
    ensure!(status == 0, "whisper-rs returned error status {status}");
    Ok(())
  }

  fn segments(&self) -> Result<Vec<TranscribedSegment>> {
    // SAFETY: state is live and was successfully transcribed before this call.
    let segment_count = unsafe { whisper_rs_sys::whisper_full_n_segments_from_state(self.state) };
    ensure!(
      segment_count >= 0,
      "whisper-rs returned a negative segment count"
    );

    let mut segments = Vec::with_capacity(segment_count as usize);
    for index in 0 .. segment_count {
      // SAFETY: index is within 0..segment_count. whisper.cpp returns a pointer
      // valid until the next transcription on this state, and we copy it.
      let text = unsafe {
        let text_ptr = whisper_rs_sys::whisper_full_get_segment_text_from_state(self.state, index);
        ensure!(
          !text_ptr.is_null(),
          "whisper-rs returned null text for segment {index}"
        );
        CStr::from_ptr(text_ptr)
          .to_str()
          .context("failed to read whisper segment text")?
          .to_owned()
      };
      // SAFETY: index is within 0..segment_count for this live state.
      let start_timestamp =
        unsafe { whisper_rs_sys::whisper_full_get_segment_t0_from_state(self.state, index) };
      // SAFETY: index is within 0..segment_count for this live state.
      let end_timestamp =
        unsafe { whisper_rs_sys::whisper_full_get_segment_t1_from_state(self.state, index) };
      segments.push(TranscribedSegment {
        start_timestamp,
        end_timestamp,
        text,
      });
    }

    Ok(segments)
  }
}

impl Drop for RawWhisperState<'_> {
  fn drop(&mut self) {
    // SAFETY: state was returned by whisper_init_state and is freed exactly
    // once here before the borrowed context can be dropped.
    unsafe {
      whisper_rs_sys::whisper_free_state(self.state);
    }
  }
}

struct TranscribedSegment {
  start_timestamp: i64,
  end_timestamp: i64,
  text: String,
}

fn whisper_language(language: &str) -> Result<Option<CString>> {
  if language.eq_ignore_ascii_case("auto") {
    return Ok(None);
  }

  CString::new(language)
    .with_context(|| format!("WHISPER_LANG contains a null byte: {language}"))
    .map(Some)
}

fn whisper_initial_prompt() -> Result<CString> {
  let prompt = env::var(WHISPER_PROMPT_ENV).unwrap_or_default();
  CString::new(prompt).with_context(|| format!("{WHISPER_PROMPT_ENV} contains a null byte"))
}

fn whisper_cli_full_params(
  whisper_threads: c_int,
  language: Option<&CStr>,
  initial_prompt: &CStr,
) -> whisper_rs_sys::whisper_full_params {
  // SAFETY: returns a plain parameter value initialized by whisper.cpp.
  let mut params = unsafe {
    whisper_rs_sys::whisper_full_default_params(
      whisper_rs_sys::whisper_sampling_strategy_WHISPER_SAMPLING_GREEDY,
    )
  };

  params.strategy = whisper_rs_sys::whisper_sampling_strategy_WHISPER_SAMPLING_BEAM_SEARCH;
  params.print_realtime = false;
  params.print_progress = false;
  params.print_timestamps = true;
  params.print_special = false;
  params.initial_prompt = initial_prompt.as_ptr();
  params.language = language.map_or(std::ptr::null(), |language| {
    language.as_ptr() as *const c_char
  });
  params.detect_language = language.is_none();
  params.n_threads = whisper_threads;
  params.token_timestamps = false;
  params.thold_pt = 0.01;
  params.max_len = 0;
  params.split_on_word = false;
  params.audio_ctx = 0;
  params.greedy.best_of = WHISPER_CLI_BEST_OF;
  params.beam_search.beam_size = WHISPER_CLI_BEAM_SIZE;
  params.beam_search.patience = -1.0;
  params.temperature = WHISPER_TEMPERATURE;
  params.temperature_inc = WHISPER_TEMPERATURE_INC;
  params.entropy_thold = WHISPER_ENTROPY_THOLD;
  params.logprob_thold = WHISPER_LOGPROB_THOLD;
  params.no_speech_thold = WHISPER_NO_SPEECH_THOLD;
  params.no_timestamps = false;
  params.suppress_nst = false;

  params
}

fn read_wav_samples(wav_file: &Path) -> Result<Vec<f32>> {
  let reader = hound::WavReader::open(wav_file)
    .with_context(|| format!("failed to open wav file {}", wav_file.display()))?;
  let spec = reader.spec();
  ensure!(
    spec.sample_rate == WHISPER_SAMPLE_RATE,
    "wav sample rate must be {WHISPER_SAMPLE_RATE} Hz: {} has {} Hz",
    wav_file.display(),
    spec.sample_rate
  );
  ensure!(
    spec.channels == 1,
    "wav must be mono: {} has {} channels",
    wav_file.display(),
    spec.channels
  );
  ensure!(
    spec.sample_format == hound::SampleFormat::Int && spec.bits_per_sample == 16,
    "wav must be 16-bit PCM: {} has {:?}/{} bits",
    wav_file.display(),
    spec.sample_format,
    spec.bits_per_sample
  );

  let samples = reader
    .into_samples::<i16>()
    .collect::<std::result::Result<Vec<_>, _>>()
    .with_context(|| format!("failed to read wav samples from {}", wav_file.display()))?;
  let mut audio = vec![0.0f32; samples.len()];
  whisper_rs::convert_integer_to_float_audio(&samples, &mut audio)
    .map_err(|err| anyhow::anyhow!("failed to convert wav samples to f32: {err}"))?;

  Ok(audio)
}

fn write_srt(state: &RawWhisperState<'_>, output: &Path) -> Result<()> {
  let mut file = fs::File::create(output)
    .with_context(|| format!("failed to create SRT file {}", output.display()))?;
  let segments = state.segments()?;

  for (index, segment) in segments.iter().enumerate() {
    writeln!(file, "{}", index + 1)
      .with_context(|| format!("failed to write SRT file {}", output.display()))?;
    writeln!(
      file,
      "{} --> {}",
      format_srt_timestamp(segment.start_timestamp),
      format_srt_timestamp(segment.end_timestamp)
    )
    .with_context(|| format!("failed to write SRT file {}", output.display()))?;
    writeln!(file, "{}", segment.text)
      .with_context(|| format!("failed to write SRT file {}", output.display()))?;
    writeln!(file).with_context(|| format!("failed to write SRT file {}", output.display()))?;
  }

  ensure!(
    !segments.is_empty(),
    "whisper-rs did not produce any segments"
  );
  Ok(())
}

fn format_srt_timestamp(timestamp_cs: i64) -> String {
  let mut millis = timestamp_cs.saturating_mul(10);
  if millis < 0 {
    millis = 0;
  }

  let hours = millis / 3_600_000;
  millis -= hours * 3_600_000;
  let minutes = millis / 60_000;
  millis -= minutes * 60_000;
  let seconds = millis / 1_000;
  millis -= seconds * 1_000;

  format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn ensure_command_available(name: &str) -> Result<()> {
  let status = Command::new(name)
    .arg("-version")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();

  match status {
    Ok(_) => Ok(()),
    Err(err) if err.kind() == ErrorKind::NotFound => bail!("`{name}` was not found in PATH"),
    Err(err) => Err(err).with_context(|| format!("failed to check `{name}` availability")),
  }
}

fn run_command(command: &mut Command, command_name: &str) -> Result<()> {
  let output = command
    .output()
    .with_context(|| format!("failed to run {command_name}"))?;
  ensure_success_output(&output, command_name)
}

fn ensure_success_output(output: &Output, command_name: &str) -> Result<()> {
  if output.status.success() {
    Ok(())
  } else {
    let stderr = output_tail(&output.stderr, COMMAND_ERROR_TAIL_LINES);
    if stderr.is_empty() {
      bail!("{command_name} exited with status {}", output.status)
    }

    bail!(
      "{command_name} exited with status {}\n{stderr}",
      output.status
    )
  }
}

fn output_tail(output: &[u8], max_lines: usize) -> String {
  let text = String::from_utf8_lossy(output);
  let mut lines = text.lines().rev().take(max_lines).collect::<Vec<_>>();
  lines.reverse();
  lines.join("\n")
}
