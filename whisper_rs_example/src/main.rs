use std::{
  env, fs,
  io::{ErrorKind, Write},
  path::{Path, PathBuf},
  process::{Command, ExitStatus, Stdio},
  sync::{Arc, mpsc},
  thread,
};

use anyhow::{Context, Result, bail, ensure};
use many_cpus::SystemHardware;
use tempfile::Builder;
use walkdir::WalkDir;
use whisper_rs::{
  FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

const DEFAULT_BACKUP_DIR: &str = "backup-20251212";
const DEFAULT_WHISPER_MODEL: &str = "/Users/gerald/test/cpp/whisper.cpp/models/ggml-base.en.bin";
const JOBS_ENV: &str = "MP4S_TO_SRT_JOBS";
const WHISPER_THREADS_ENV: &str = "WHISPER_THREADS";
const WHISPER_SAMPLE_RATE: u32 = 16_000;
const DEFAULT_MAX_WHISPER_THREADS_PER_TASK: usize = 4;

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
struct TaskResult {
  input: PathBuf,
  output: PathBuf,
  result: Result<()>,
}

fn main() -> Result<()> {
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
      "No mp4 files need SRT generation under: {}",
      config.scan_dir.display()
    );
    return Ok(());
  }

  let jobs = config.jobs.min(tasks.len());
  let whisper_threads = config
    .whisper_threads
    .unwrap_or_else(|| default_whisper_threads(jobs));
  eprintln!(
    "Processing {} mp4 file(s) with {jobs} worker thread(s), {whisper_threads} whisper thread(s) \
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
    let model = env::var_os("WHISPER_MODEL")
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from(DEFAULT_WHISPER_MODEL));
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
    return Ok(PathBuf::from(path));
  }

  let home = env::var_os("HOME").context("HOME is not set")?;
  Ok(PathBuf::from(home).join(DEFAULT_BACKUP_DIR))
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
    if !entry.file_type().is_file() || !is_mp4(path) {
      continue;
    }

    if let Some(task) = task_for_input(path, config)? {
      tasks.push(task);
    }
  }

  Ok(tasks)
}

fn task_for_input(input: &Path, config: &Config) -> Result<Option<Task>> {
  let sibling_output = input.with_extension("srt");
  if sibling_output.is_file() {
    eprintln!("Skipping existing SRT: {}", sibling_output.display());
    return Ok(None);
  }

  let input_dir = input
    .parent()
    .with_context(|| format!("failed to read parent directory for {}", input.display()))?;

  let output = if can_create_file_in(input_dir) {
    sibling_output
  } else {
    let relative_input = input.strip_prefix(&config.scan_dir).unwrap_or(input);
    let output = config.backup_dir.join(relative_input).with_extension("srt");
    if output.is_file() {
      eprintln!("Skipping existing SRT: {}", output.display());
      return Ok(None);
    }
    output
  };

  Ok(Some(Task {
    input: input.to_path_buf(),
    output,
  }))
}

fn is_mp4(path: &Path) -> bool {
  path
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
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
  let whisper_context = Arc::new(
    WhisperContext::new_with_params(&config.model, WhisperContextParameters::default())
      .with_context(|| format!("failed to load whisper model {}", config.model.display()))?,
  );

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
      Ok(()) => eprintln!("Generated: {}", task_result.output.display()),
      Err(err) => {
        failed += 1;
        eprintln!("Failed: {}", task_result.input.display());
        eprintln!("  {err:#}");
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
  whisper_context: &WhisperContext,
  task: &Task,
  whisper_threads: usize,
) -> Result<()> {
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
  extract_audio(&task.input, &wav_path)?;
  ensure_complete_audio(&task.input, &wav_path)?;

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
  fs::rename(&generated_srt, &task.output).with_context(|| {
    format!(
      "failed to move generated SRT from {} to {}",
      generated_srt.display(),
      task.output.display()
    )
  })?;

  Ok(())
}

fn extract_audio(input: &Path, wav_file: &Path) -> Result<()> {
  let status = Command::new("ffmpeg")
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
    .arg(wav_file)
    .status()
    .with_context(|| "failed to run ffmpeg")?;

  ensure_success(status, "ffmpeg")
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

  ensure_success(output.status, "ffprobe")?;

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
  whisper_context: &WhisperContext,
  wav_file: &Path,
  output: &Path,
  whisper_threads: usize,
) -> Result<()> {
  let audio = read_wav_samples(wav_file)?;
  let mut state = whisper_context
    .create_state()
    .context("failed to create whisper state")?;

  let mut params = FullParams::new(SamplingStrategy::BeamSearch {
    beam_size: 5,
    patience: -1.0,
  });
  params.set_n_threads(
    i32::try_from(whisper_threads).context("WHISPER_THREADS is too large for whisper-rs")?,
  );
  if config.language.eq_ignore_ascii_case("auto") {
    params.set_language(None);
    params.set_detect_language(true);
  } else {
    params.set_language(Some(&config.language));
  }
  params.set_print_special(false);
  params.set_print_progress(false);
  params.set_print_realtime(false);
  params.set_print_timestamps(false);

  state
    .full(params, &audio)
    .with_context(|| format!("failed to transcribe {}", wav_file.display()))?;
  write_srt(&state, output)
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

fn write_srt(state: &WhisperState, output: &Path) -> Result<()> {
  let mut file = fs::File::create(output)
    .with_context(|| format!("failed to create SRT file {}", output.display()))?;
  let mut segment_count = 0usize;

  for (index, segment) in state.as_iter().enumerate() {
    segment_count += 1;
    let text = segment
      .to_str_lossy()
      .context("failed to read whisper segment text")?;
    writeln!(file, "{}", index + 1)
      .with_context(|| format!("failed to write SRT file {}", output.display()))?;
    writeln!(
      file,
      "{} --> {}",
      format_srt_timestamp(segment.start_timestamp()),
      format_srt_timestamp(segment.end_timestamp())
    )
    .with_context(|| format!("failed to write SRT file {}", output.display()))?;
    writeln!(file, "{text}")
      .with_context(|| format!("failed to write SRT file {}", output.display()))?;
    writeln!(file).with_context(|| format!("failed to write SRT file {}", output.display()))?;
  }

  ensure!(segment_count > 0, "whisper-rs did not produce any segments");
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

fn ensure_success(status: ExitStatus, command_name: &str) -> Result<()> {
  if status.success() {
    Ok(())
  } else {
    bail!("{command_name} exited with status {status}")
  }
}
