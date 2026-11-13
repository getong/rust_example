use std::{path::Path, process::Command, time::Duration};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use crate::process;

pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov", "mkv", "webm", "avi"];

#[derive(Clone, Debug)]
pub struct Track {
  pub index: u32,
  pub label: String,
  pub text: bool,
}

#[derive(Clone, Debug)]
pub struct Chapter {
  pub title: String,
  pub start: Duration,
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
  pub duration: Duration,
  pub width: u32,
  pub height: u32,
  pub fps: f64,
  pub video_index: u32,
  pub rotation: i32,
  pub color_space: String,
  pub full_range: bool,
  pub hardware_compatible: bool,
  pub audio_tracks: Vec<Track>,
  pub subtitle_tracks: Vec<Track>,
  pub chapters: Vec<Chapter>,
}

pub fn supported(path: &Path) -> bool {
  path
    .extension()
    .and_then(|value| value.to_str())
    .is_some_and(|value| {
      SUPPORTED_VIDEO_EXTENSIONS
        .iter()
        .any(|extension| value.eq_ignore_ascii_case(extension))
    })
}

impl MediaInfo {
  pub fn probe(path: &Path) -> Result<Self> {
    ensure!(supported(path), "unsupported media file extension");
    ensure!(
      path.is_file(),
      "media file does not exist: {}",
      path.display()
    );
    let result = process::output(
      Command::new("ffprobe")
        .args([
          "-v",
          "error",
          "-show_streams",
          "-show_format",
          "-show_chapters",
          "-of",
          "json",
        ])
        .arg(path),
      Duration::from_secs(20),
    )?;
    ensure!(
      result.status.success(),
      "could not inspect media: {}",
      String::from_utf8_lossy(&result.stderr)
    );
    let document: Value =
      serde_json::from_slice(&result.stdout).context("invalid ffprobe metadata")?;
    let streams = document["streams"]
      .as_array()
      .context("media contains no streams")?;
    let video = streams
      .iter()
      .find(|stream| {
        stream["codec_type"] == "video"
          && stream["disposition"]["attached_pic"].as_u64().unwrap_or(0) == 0
      })
      .context("media contains no video track")?;
    let width = video["width"]
      .as_u64()
      .and_then(|value| u32::try_from(value).ok())
      .context("invalid video width")?;
    let height = video["height"]
      .as_u64()
      .and_then(|value| u32::try_from(value).ok())
      .context("invalid video height")?;
    ensure!(
      width > 0 && height > 0 && u64::from(width) * u64::from(height) <= 64 * 1024 * 1024,
      "video dimensions exceed limits"
    );
    let seconds = video["duration"]
      .as_str()
      .and_then(|value| value.parse::<f64>().ok())
      .or_else(|| {
        document["format"]["duration"]
          .as_str()
          .and_then(|value| value.parse().ok())
      })
      .context("media duration is unknown; live streams are not supported")?;
    let duration = Duration::try_from_secs_f64(seconds).context("invalid media duration")?;
    ensure!(!duration.is_zero(), "media duration is zero");
    let rate = video["avg_frame_rate"].as_str().unwrap_or("0/1");
    let fps = rate
      .split_once('/')
      .and_then(|(numerator, denominator)| {
        Some(numerator.parse::<f64>().ok()? / denominator.parse::<f64>().ok()?)
      })
      .filter(|value| value.is_finite() && *value > 0.0)
      .unwrap_or(30.0);
    let rotation = video["side_data_list"]
      .as_array()
      .and_then(|items| items.iter().find_map(|item| item["rotation"].as_i64()))
      .or_else(|| {
        video["tags"]["rotate"]
          .as_str()
          .and_then(|value| value.parse().ok())
      })
      .unwrap_or(0) as i32;
    let color_space = video["color_space"]
      .as_str()
      .unwrap_or("unknown")
      .to_owned();
    let hardware_compatible = video["codec_name"] == "h264"
      && matches!(
        video["pix_fmt"].as_str(),
        Some("yuv420p" | "yuvj420p" | "nv12")
      )
      && rotation.rem_euclid(360) == 0
      && matches!(
        color_space.as_str(),
        "bt709" | "smpte170m" | "bt470bg" | "unknown"
      );
    let tracks = |kind: &str| {
      streams
        .iter()
        .filter(|stream| stream["codec_type"] == kind)
        .filter_map(|stream| {
          let index = u32::try_from(stream["index"].as_u64()?).ok()?;
          let codec = stream["codec_name"].as_str().unwrap_or("unknown");
          let language = stream["tags"]["language"].as_str().unwrap_or("und");
          let title = stream["tags"]["title"].as_str().unwrap_or("");
          Some(Track {
            index,
            label: format!("#{index} {language} {codec} {title}"),
            text: matches!(
              codec,
              "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "mov_text" | "text"
            ),
          })
        })
        .collect::<Vec<_>>()
    };
    Ok(Self {
      duration,
      width,
      height,
      fps,
      video_index: video["index"]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .context("invalid video index")?,
      rotation,
      color_space,
      full_range: video["color_range"] == "pc",
      hardware_compatible,
      audio_tracks: tracks("audio"),
      subtitle_tracks: tracks("subtitle"),
      chapters: document["chapters"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, chapter)| {
          let seconds = chapter["start_time"].as_str()?.parse().ok()?;
          let start = Duration::try_from_secs_f64(seconds).ok()?;
          (start < duration).then(|| Chapter {
            title: chapter["tags"]["title"]
              .as_str()
              .map(str::to_owned)
              .unwrap_or_else(|| format!("第 {} 章", index + 1)),
            start,
          })
        })
        .collect(),
    })
  }

  pub fn display_size(&self) -> (u32, u32) {
    if self.rotation.rem_euclid(180) == 90 {
      (self.height, self.width)
    } else {
      (self.width, self.height)
    }
  }
}
