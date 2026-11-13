use std::{
  io::Read,
  path::Path,
  process::{Child, Command, Stdio},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
  },
  thread,
  time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};

pub fn tempo_filter(speed: f32) -> String {
  if speed > 2.0 {
    format!("atempo=2,atempo={}", speed / 2.0)
  } else {
    format!("atempo={speed}")
  }
}

struct AudioSource {
  child: Child,
  buffers: Receiver<Result<Vec<f32>>>,
  buffer: Vec<f32>,
  next_sample: usize,
  finished: bool,
  errors: Sender<String>,
}

impl AudioSource {
  fn open(
    path: &Path,
    track: u32,
    position: Duration,
    speed: f32,
    errors: Sender<String>,
  ) -> Result<Self> {
    let mut child = Command::new("ffmpeg")
      .args(["-nostdin", "-v", "error", "-ss"])
      .arg(format!("{:.9}", position.as_secs_f64()))
      .arg("-i")
      .arg(path)
      .args([
        "-map",
        &format!("0:{track}"),
        "-vn",
        "-sn",
        "-dn",
        "-af",
        &tempo_filter(speed),
        "-ac",
        "2",
        "-ar",
        "48000",
        "-f",
        "f32le",
        "pipe:1",
      ])
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .context("could not start FFmpeg audio decoding")?;
    let mut stdout = child.stdout.take().context("missing audio pipe")?;
    let mut stderr = child.stderr.take().context("missing audio diagnostics")?;
    let error_sender = errors.clone();
    thread::spawn(move || {
      let mut bytes = Vec::new();
      let _ = stderr.by_ref().take(16384).read_to_end(&mut bytes);
      if !bytes.is_empty() {
        let _ = error_sender.send(String::from_utf8_lossy(&bytes).into_owned());
      }
    });
    let (sender, buffers) = mpsc::sync_channel(8);
    thread::spawn(move || {
      loop {
        let mut bytes = vec![0; 8192];
        let mut filled = 0;
        while filled < bytes.len() {
          match stdout.read(&mut bytes[filled ..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
              let _ = sender.send(Err(error).context("could not read audio samples"));
              return;
            }
          }
        }
        if filled == 0 {
          break;
        }
        bytes.truncate(filled);
        if !filled.is_multiple_of(8) {
          let _ = sender.send(Err(anyhow::anyhow!("incomplete audio frame")));
          break;
        }
        let samples = bytes
          .as_chunks::<4>()
          .0
          .iter()
          .map(|bytes| f32::from_le_bytes(*bytes))
          .collect();
        if sender.send(Ok(samples)).is_err() {
          break;
        }
      }
    });
    let mut source = Self {
      child,
      buffers,
      buffer: Vec::new(),
      next_sample: 0,
      finished: false,
      errors,
    };
    ensure!(source.refill()?, "audio decoder produced no samples");
    Ok(source)
  }

  fn refill(&mut self) -> Result<bool> {
    match self.buffers.recv_timeout(Duration::from_secs(5)) {
      Ok(buffer) => {
        self.buffer = buffer?;
        self.next_sample = 0;
        Ok(!self.buffer.is_empty())
      }
      Err(mpsc::RecvTimeoutError::Disconnected) => {
        let status = self.child.wait()?;
        ensure!(status.success(), "FFmpeg audio decoder failed: {status}");
        Ok(false)
      }
      Err(error) => Err(error).context("audio decoding timed out"),
    }
  }
}

impl Iterator for AudioSource {
  type Item = f32;
  fn next(&mut self) -> Option<f32> {
    if self.finished {
      return None;
    }
    if self.next_sample == self.buffer.len() {
      match self.refill() {
        Ok(true) => {}
        result => {
          self.finished = true;
          if let Err(error) = result {
            let _ = self.errors.send(format!("{error:#}"));
          }
          return None;
        }
      }
    }
    let sample = self.buffer[self.next_sample];
    self.next_sample += 1;
    Some(sample)
  }
}

impl Source for AudioSource {
  fn current_span_len(&self) -> Option<usize> {
    Some(if self.finished { 0 } else { self.buffer.len() })
  }
  fn channels(&self) -> ChannelCount {
    ChannelCount::new(2).unwrap_or(ChannelCount::MIN)
  }
  fn sample_rate(&self) -> SampleRate {
    SampleRate::new(48000).unwrap_or(SampleRate::MIN)
  }
  fn total_duration(&self) -> Option<Duration> {
    None
  }
}

impl Drop for AudioSource {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

pub struct AudioPlayback {
  player: Player,
  errors: Receiver<String>,
  media_position_micros: Arc<AtomicU64>,
  position_offset: Duration,
  paused: bool,
  output: MixerDeviceSink,
}

impl AudioPlayback {
  pub fn new(path: &Path, track: u32, position: Duration, speed: f32) -> Result<Self> {
    let mut output = DeviceSinkBuilder::open_default_sink().context("audio output unavailable")?;
    output.log_on_drop(false);
    let player = Player::connect_new(output.mixer());
    player.pause();
    let (errors, media_position_micros) =
      Self::append_source(&player, path, track, position, speed)?;
    Ok(Self {
      player,
      errors,
      media_position_micros,
      position_offset: position,
      paused: true,
      output,
    })
  }

  fn append_source(
    player: &Player,
    path: &Path,
    track: u32,
    offset: Duration,
    speed: f32,
  ) -> Result<(Receiver<String>, Arc<AtomicU64>)> {
    let (sender, errors) = mpsc::channel();
    let source = AudioSource::open(path, track, offset, speed, sender)?;
    let media_position_micros = Arc::new(AtomicU64::new(0));
    let position = Arc::clone(&media_position_micros);
    player.append(source.track_position().periodic_access(
      Duration::from_millis(5),
      move |source| {
        position.store(
          source.get_pos().mul_f64(f64::from(speed)).as_micros() as u64,
          Ordering::Relaxed,
        );
      },
    ));
    Ok((errors, media_position_micros))
  }

  pub fn set_paused(&mut self, paused: bool) {
    self.paused = paused;
    if paused {
      self.player.pause();
    } else {
      self.player.play();
    }
  }

  pub fn seek(&mut self, path: &Path, track: u32, position: Duration, speed: f32) -> Result<()> {
    let player = Player::connect_new(self.output.mixer());
    player.pause();
    player.set_volume(self.player.volume());
    let (errors, media_position_micros) =
      Self::append_source(&player, path, track, position, speed)?;
    self.player = player;
    self.errors = errors;
    self.media_position_micros = media_position_micros;
    self.position_offset = position;
    self.set_paused(self.paused);
    Ok(())
  }

  pub fn set_volume(&self, volume: f32) {
    self.player.set_volume(volume.clamp(0.0, 1.0));
  }
  pub fn position(&self) -> Option<Duration> {
    (!self.player.empty()).then(|| {
      self.position_offset
        + Duration::from_micros(self.media_position_micros.load(Ordering::Relaxed))
    })
  }
  pub fn check_error(&self) -> Result<()> {
    if let Ok(error) = self.errors.try_recv() {
      bail!("audio playback failed: {error}");
    }
    Ok(())
  }
}
