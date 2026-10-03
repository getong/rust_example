use std::{
  collections::BTreeSet,
  env,
  ffi::OsString,
  fs::{self, OpenOptions},
  io,
  path::{Path, PathBuf},
  process::{Command, ExitCode},
  sync::atomic::{AtomicU64, Ordering},
};

const HELP: &str = "递归将 WebVTT 转换为同目录的 SRT（保留原文件）

用法: convert_vtt_srt [选项] [文件或目录 ...]

不指定路径时扫描当前工作目录，包含隐藏目录，不跟随符号链接。
默认跳过已有 SRT；转换依赖 PATH 中的 ffmpeg。

选项:
  -n, --dry-run       只显示待转换文件，不写入文件
  -f, --overwrite     覆盖已有 SRT
      --ffmpeg PATH   指定 ffmpeg 可执行文件
  -h, --help          显示帮助
      --              后续参数均作为路径
";

#[derive(Debug)]
struct Options {
  paths: Vec<PathBuf>,
  ffmpeg: OsString,
  dry_run: bool,
  overwrite: bool,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Option<Options>, String> {
  let mut options = Options {
    paths: vec![],
    ffmpeg: "ffmpeg".into(),
    dry_run: false,
    overwrite: false,
  };
  let mut args = args.into_iter();
  let mut positional = false;
  while let Some(arg) = args.next() {
    if positional {
      options.paths.push(arg.into());
      continue;
    }
    match arg.to_str() {
      Some("-h" | "--help") => return Ok(None),
      Some("-n" | "--dry-run") => options.dry_run = true,
      Some("-f" | "--overwrite") => options.overwrite = true,
      Some("--") => positional = true,
      Some("--ffmpeg") => {
        options.ffmpeg = args.next().ok_or("--ffmpeg 缺少可执行文件路径")?;
      }
      Some(s) if s.starts_with('-') => return Err(format!("未知选项: {s}")),
      _ => options.paths.push(arg.into()),
    }
  }
  if options.paths.is_empty() {
    options.paths.push(PathBuf::from("."));
  }
  Ok(Some(options))
}

fn is_vtt(path: &Path) -> bool {
  path
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case("vtt"))
}

fn collect(path: &Path, files: &mut BTreeSet<PathBuf>, errors: &mut usize) {
  let result = (|| -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
      for entry in fs::read_dir(path)? {
        match entry {
          Ok(entry) => collect(&entry.path(), files, errors),
          Err(error) => {
            eprintln!("无法读取目录项 {}: {error}", path.display());
            *errors += 1;
          }
        }
      }
    } else if metadata.is_file() && is_vtt(path) {
      files.insert(fs::canonicalize(path)?);
    }
    Ok(())
  })();
  if let Err(error) = result {
    eprintln!("无法扫描 {}: {error}", path.display());
    *errors += 1;
  }
}

// A sibling temporary file keeps publication on the same filesystem.
// Drop removes partial output when ffmpeg or publication fails.
struct TemporaryOutput(PathBuf);

impl TemporaryOutput {
  fn create(parent: &Path) -> io::Result<Self> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    loop {
      let id = COUNTER.fetch_add(1, Ordering::Relaxed);
      let path = parent.join(format!(".vtt-srt-{}-{id}.tmp", std::process::id()));
      match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(_) => return Ok(Self(path)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
        Err(error) => return Err(error),
      }
    }
  }
}

impl Drop for TemporaryOutput {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.0);
  }
}

fn convert(input: &Path, output: &Path, options: &Options) -> Result<(), String> {
  let temporary = TemporaryOutput::create(output.parent().unwrap_or(Path::new(".")))
    .map_err(|error| format!("无法创建临时文件: {error}"))?;
  let result = Command::new(&options.ffmpeg)
    .args([
      "-nostdin",
      "-hide_banner",
      "-loglevel",
      "error",
      "-y",
      "-f",
      "webvtt",
      "-i",
    ])
    .arg(input)
    .args(["-map", "0:s:0", "-c:s", "srt", "-f", "srt"])
    .arg(&temporary.0)
    .output()
    .map_err(|error| format!("无法执行 {:?}: {error}", options.ffmpeg))?;
  if !result.status.success() {
    return Err(format!(
      "ffmpeg {}: {}",
      result.status,
      String::from_utf8_lossy(&result.stderr).trim()
    ));
  }
  if fs::metadata(&temporary.0)
    .map_err(|error| format!("无法检查转换结果: {error}"))?
    .len()
    == 0
  {
    return Err("未生成任何字幕，输入可能为空或无效；目标文件保持不变".into());
  }
  if options.overwrite {
    fs::rename(&temporary.0, output)
  } else {
    // Unlike a check followed by rename, hard_link cannot overwrite a file
    // created by another process while ffmpeg is running.
    fs::hard_link(&temporary.0, output)
  }
  .map_err(|error| format!("无法保存 SRT: {error}"))
}

fn run(options: Options) -> ExitCode {
  let mut files = BTreeSet::new();
  let mut errors = 0;
  for path in &options.paths {
    collect(path, &mut files, &mut errors);
  }
  let mut completed = 0;
  let mut skipped = 0;
  for input in &files {
    let output = input.with_extension("srt");
    match fs::symlink_metadata(&output) {
      Ok(_) if !options.overwrite => {
        println!("跳过（目标已存在）: {}", output.display());
        skipped += 1;
        continue;
      }
      Err(error) if error.kind() != io::ErrorKind::NotFound => {
        eprintln!("无法检查 {}: {error}", output.display());
        errors += 1;
        continue;
      }
      _ => {}
    }
    if options.dry_run {
      println!("预览: {} -> {}", input.display(), output.display());
      completed += 1;
    } else {
      match convert(input, &output, &options) {
        Ok(()) => {
          println!("完成: {} -> {}", input.display(), output.display());
          completed += 1;
        }
        Err(error) => {
          eprintln!("转换失败 {}: {error}", input.display());
          errors += 1;
        }
      }
    }
  }
  println!(
    "找到 {} 个 VTT，{} {} 个，跳过 {skipped} 个，错误 {errors} 个。",
    files.len(),
    if options.dry_run {
      "待转换"
    } else {
      "已转换"
    },
    completed
  );
  if errors == 0 {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

fn main() -> ExitCode {
  match parse_args(env::args_os().skip(1)) {
    Ok(Some(options)) => run(options),
    Ok(None) => {
      print!("{HELP}");
      ExitCode::SUCCESS
    }
    Err(error) => {
      eprintln!("{error}\n\n{HELP}");
      ExitCode::from(2)
    }
  }
}
