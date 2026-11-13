use std::{
  fs,
  path::{Path, PathBuf},
  time::Duration,
};

use anyhow::{Context as _, Result, bail, ensure};

pub const SUPPORTED_SUBTITLE_EXTENSIONS: &[&str] = &["srt"];

struct SubtitleCue {
  start: Duration,
  end: Duration,
  text: String,
}

pub struct SubtitleTrack {
  path: PathBuf,
  cues: Vec<SubtitleCue>,
  latest_ends: Vec<Duration>,
}

impl SubtitleTrack {
  pub fn load(path: &Path) -> Result<Self> {
    ensure!(
      path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("srt")),
      "only .srt subtitle files are supported"
    );
    let bytes = fs::read(path)
      .with_context(|| format!("could not read subtitles from {}", path.display()))?;
    let source = decode_text(&bytes)?;
    let source = source
      .trim_start_matches('\u{feff}')
      .replace("\r\n", "\n")
      .replace('\r', "\n");
    let mut cues = Vec::new();
    let mut block = Vec::new();
    for line in source.lines().chain(std::iter::once("")) {
      if line.trim().is_empty() {
        if !block.is_empty() {
          cues.push(
            parse_cue(&block)
              .with_context(|| format!("invalid SRT subtitle block {}", cues.len() + 1))?,
          );
          block.clear();
        }
      } else {
        block.push(line);
      }
    }
    ensure!(!cues.is_empty(), "the SRT file contains no subtitles");
    cues.sort_by_key(|cue| cue.start);
    let mut latest_end = Duration::ZERO;
    let latest_ends = cues
      .iter()
      .map(|cue| {
        latest_end = latest_end.max(cue.end);
        latest_end
      })
      .collect();
    Ok(Self {
      path: path.to_path_buf(),
      cues,
      latest_ends,
    })
  }

  pub fn for_video(video: &Path) -> Result<Option<Self>> {
    let candidate = video.with_extension("srt");
    if candidate.try_exists()? {
      return Self::load(&candidate).map(Some);
    }
    let directory = video
      .parent()
      .filter(|parent| !parent.as_os_str().is_empty())
      .unwrap_or_else(|| Path::new("."));
    for entry in fs::read_dir(directory)? {
      let path = entry?.path();
      if path.file_stem() == video.file_stem()
        && path
          .extension()
          .and_then(|extension| extension.to_str())
          .is_some_and(|extension| extension.eq_ignore_ascii_case("srt"))
        && path.is_file()
      {
        return Self::load(&path).map(Some);
      }
    }
    Ok(None)
  }

  pub fn path(&self) -> &Path {
    &self.path
  }

  pub fn text_at(&self, time: Duration) -> Option<String> {
    let end = self.cues.partition_point(|cue| cue.start <= time);
    let start = self.latest_ends[.. end].partition_point(|latest| *latest <= time);
    let text = self.cues[start .. end]
      .iter()
      .filter(|cue| time < cue.end)
      .map(|cue| cue.text.as_str())
      .collect::<Vec<_>>()
      .join("\n");
    (!text.is_empty()).then_some(text)
  }
}

fn decode_text(bytes: &[u8]) -> Result<String> {
  let little_endian = bytes.starts_with(&[0xff, 0xfe]);
  if little_endian || bytes.starts_with(&[0xfe, 0xff]) {
    ensure!(
      bytes.len().is_multiple_of(2),
      "invalid UTF-16 subtitle data"
    );
    let units = bytes[2 ..]
      .as_chunks::<2>()
      .0
      .iter()
      .map(|pair| {
        if little_endian {
          u16::from_le_bytes(*pair)
        } else {
          u16::from_be_bytes(*pair)
        }
      })
      .collect::<Vec<_>>();
    return String::from_utf16(&units).context("invalid UTF-16 subtitle text");
  }
  String::from_utf8(bytes.to_vec())
    .context("unsupported subtitle encoding; save the SRT file as UTF-8 or UTF-16 with a BOM")
}

fn parse_cue(lines: &[&str]) -> Result<SubtitleCue> {
  let (first, rest) = lines.split_first().context("missing subtitle timing")?;
  let (timing, text) = if first.trim().bytes().all(|byte| byte.is_ascii_digit()) {
    rest.split_first().context("missing subtitle timing")?
  } else {
    (first, rest)
  };
  let (start, end) = timing
    .split_once("-->")
    .context("expected start --> end timing")?;
  let start = parse_timestamp(start.trim())?;
  let end = parse_timestamp(end.split_whitespace().next().context("missing end time")?)?;
  ensure!(end > start, "subtitle end must be after its start");
  ensure!(!text.is_empty(), "missing subtitle text");
  Ok(SubtitleCue {
    start,
    end,
    text: text.join("\n"),
  })
}

fn parse_timestamp(timestamp: &str) -> Result<Duration> {
  let Some((clock, millis)) = timestamp.split_once([',', '.']) else {
    bail!("invalid subtitle timestamp: {timestamp}");
  };
  let parts = clock.split(':').collect::<Vec<_>>();
  ensure!(
    parts.len() == 3
      && parts
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
      && millis.len() == 3
      && millis.bytes().all(|byte| byte.is_ascii_digit()),
    "expected HH:MM:SS,mmm timestamp, got {timestamp}"
  );
  let hours = parts[0].parse::<u64>()?;
  let minutes = parts[1].parse::<u64>()?;
  let seconds = parts[2].parse::<u64>()?;
  let millis = millis.parse::<u64>()?;
  ensure!(
    minutes < 60 && seconds < 60,
    "invalid subtitle timestamp: {timestamp}"
  );
  let seconds = hours
    .checked_mul(3600)
    .and_then(|total| total.checked_add(minutes * 60 + seconds))
    .context("subtitle timestamp is too large")?;
  Ok(Duration::new(seconds, (millis * 1_000_000) as u32))
}
