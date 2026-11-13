use std::{
  fs,
  path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
  pub enabled: bool,
  pub delay: f64,
  pub size: f32,
  pub outline: bool,
  pub opacity: f32,
  pub position: u8,
}

impl Default for SubtitleSettings {
  fn default() -> Self {
    Self {
      enabled: true,
      delay: 0.0,
      size: 22.0,
      outline: true,
      opacity: 0.65,
      position: 2,
    }
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFile {
  pub path: PathBuf,
  pub position: f64,
  #[serde(default)]
  pub subtitle: Option<PathBuf>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
  pub volume: f32,
  pub muted: bool,
  pub speed: f32,
  pub repeat: bool,
  pub subtitles: SubtitleSettings,
  pub recent: Vec<RecentFile>,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      volume: 1.0,
      muted: false,
      speed: 1.0,
      repeat: false,
      subtitles: SubtitleSettings::default(),
      recent: Vec::new(),
    }
  }
}

impl Settings {
  pub fn path() -> PathBuf {
    if let Some(path) = std::env::var_os("GPUI_VIDEO_CONFIG") {
      return path.into();
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
      .map(PathBuf::from)
      .unwrap_or_else(|| {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()));
        if cfg!(target_os = "macos") {
          home.join("Library/Application Support")
        } else {
          home.join(".config")
        }
      });
    base.join("gpui-video-player/settings.json")
  }

  pub fn load(path: &Path) -> Result<Self> {
    if !path.exists() {
      return Ok(Self::default());
    }
    let mut settings: Self = serde_json::from_slice(&fs::read(path)?).context(
      "invalid player settings; defaults will be used without replacing the invalid file",
    )?;
    settings.volume = if settings.volume.is_finite() {
      settings.volume.clamp(0.0, 1.0)
    } else {
      1.0
    };
    if !crate::playback::PLAYBACK_SPEEDS.contains(&settings.speed) {
      settings.speed = 1.0;
    }
    settings.subtitles.size = settings.subtitles.size.clamp(12.0, 64.0);
    settings.subtitles.opacity = settings.subtitles.opacity.clamp(0.0, 1.0);
    settings.subtitles.delay = settings.subtitles.delay.clamp(-60.0, 60.0);
    settings.subtitles.position = settings.subtitles.position.min(2);
    settings
      .recent
      .retain(|entry| entry.position.is_finite() && entry.position >= 0.0);
    settings.recent.truncate(20);
    Ok(settings)
  }

  pub fn remember(&mut self, path: &Path, position: f64, subtitle: Option<PathBuf>) {
    self.recent.retain(|entry| entry.path != path);
    self.recent.insert(
      0,
      RecentFile {
        path: path.to_path_buf(),
        position,
        subtitle,
      },
    );
    self.recent.truncate(20);
  }

  pub fn save(&self, path: &Path) -> Result<()> {
    if let Some(parent) = path
      .parent()
      .filter(|parent| !parent.as_os_str().is_empty())
    {
      fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
    fs::rename(&temporary, path).context("could not save player settings")
  }
}
